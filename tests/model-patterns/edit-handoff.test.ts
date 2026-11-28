import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { EventEmitter, once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer, type ServerResponse } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import { promisify } from "node:util";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Request = "First" | "Second";
type Command = { kind: "commit" | "open" | "drop" | "send"; request: Request };
type Fault = "none" | "early" | "latest";
const checker = "crates/idealis-check/target/debug/idealis-check";
const dir = "examples/model-patterns/offline-merge/handoff";
const model = `${dir}/model.project.json`;
const requests = ["First", "Second"] as const;

async function startHandoff(t: TestContext, fault: Fault) {
  const scratch = await mkdtemp(join(tmpdir(), "idealis-edit-handoff-"));
  const path = join(scratch, "edits.sqlite");
  let db = new DatabaseSync(path);
  db.exec(
    "CREATE TABLE receipts (revision INTEGER PRIMARY KEY, request TEXT UNIQUE NOT NULL)",
  );
  const head = () =>
    Number(
      db
        .prepare("SELECT coalesce(max(revision), 0) AS head FROM receipts")
        .get()?.head,
    );
  const revisionFor = (request: Request) =>
    db.prepare("SELECT revision FROM receipts WHERE request = ?").get(request)
      ?.revision ?? null;
  const sockets = new Map<string, ServerResponse>();
  const pending = new Map<Request, Promise<{ revision: number }>>();
  const received = new Map<Request, number>();
  const opened = new EventEmitter();
  const server = createServer((req, res) => {
    const request = new URL(
      req.url ?? "/",
      "http://localhost",
    ).searchParams.get("request");
    if (request === null || sockets.has(request)) {
      res.writeHead(400).end();
      return;
    }
    sockets.set(request, res);
    if (fault === "early") res.end(JSON.stringify({ revision: head() }));
    opened.emit(request);
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(async () => {
    const closed = new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
    server.closeAllConnections();
    await closed;
    db.close();
    await rm(scratch, { recursive: true, force: true });
  });
  const address = server.address();
  assert.ok(address && typeof address !== "string");

  async function receive(
    request: Request,
    response: Promise<{ revision: number }>,
  ) {
    const body = await response;
    received.set(request, body.revision);
    pending.delete(request);
    sockets.delete(request);
  }

  return {
    async apply({ kind, request }: Command) {
      if (kind === "commit") {
        db.prepare("INSERT INTO receipts(request) VALUES (?)").run(request);
        return;
      }
      if (kind === "open") {
        const entered = once(opened, request);
        const response = fetch(
          `http://127.0.0.1:${address.port}/edits?request=${request}`,
          {
            method: "POST",
            signal: AbortSignal.timeout(10_000),
          },
        ).then(async (res) => {
          assert.equal(res.status, 200);
          return res.json() as Promise<{ revision: number }>;
        });
        pending.set(request, response);
        await Promise.race([entered, response]);
        if (sockets.get(request)?.writableEnded)
          await receive(request, response);
        return;
      }
      const socket = sockets.get(request);
      const response = pending.get(request);
      assert.ok(socket && response);
      if (kind === "drop") {
        socket.destroy();
        await assert.rejects(response, TypeError);
        sockets.delete(request);
        pending.delete(request);
        db.close();
        db = new DatabaseSync(path);
      } else {
        const revision = fault === "latest" ? head() : revisionFor(request);
        socket.end(JSON.stringify({ revision }));
        await receive(request, response);
      }
    },
    async observe() {
      return {
        head: head(),
        receipt: Object.fromEntries(
          requests.map((request) => [request, revisionFor(request)]),
        ),
        waiting: [...pending.keys()].sort(),
        received: Object.fromEntries(
          requests.map((request) => [request, received.get(request) ?? null]),
        ),
      };
    },
    project(state: Readonly<Record<string, unknown>>) {
      return {
        ...state,
        waiting: Array.isArray(state.waiting)
          ? [...state.waiting].sort()
          : state.waiting,
      };
    },
  };
}

test(
  "the response waits for a durable receipt and follows its original revision",
  { timeout: 30_000 },
  async (t) => {
    const [{ stdout: storage }, openOnly] = await Promise.all([
      promisify(execFile)(checker, [
        `${dir}/storage.project.json`,
        "--checker",
        "reference",
        "--question",
        "ReceiptWithinHead",
      ]),
      reachableScenario<Command>(checker, model, "WaitingForFirstReceipt"),
    ]);
    const [
      {
        answers: [storageResult],
      },
    ] = JSON.parse(storage);
    assert.equal(storageResult.verdict, "holds");
    assert.equal(storageResult.coverage.kind, "complete");
    assert.deepEqual(openOnly.commands, [{ kind: "open", request: "First" }]);

    await t.test("commit before opening a response", async (t) => {
      const result = await recordReplay<Command>(
        checker,
        {
          ...openOnly,
          commands: [
            { kind: "commit", request: "First" },
            { kind: "open", request: "First" },
            { kind: "send", request: "First" },
          ],
        },
        await startHandoff(t, "none"),
      );
      assert.equal(result.verdict, "matched");
      assert.deepEqual(result.recording.observations.at(-1), {
        head: 1,
        receipt: { First: 1, Second: null },
        waiting: [],
        received: { First: 1, Second: null },
      });
    });

    const retry: Command[] = [
      ...openOnly.commands,
      { kind: "commit", request: "First" },
      { kind: "drop", request: "First" },
      { kind: "open", request: "First" },
      { kind: "commit", request: "Second" },
      { kind: "send", request: "First" },
    ];
    for (const fault of ["none", "early", "latest"] as const) {
      await t.test(fault, async (t) => {
        const commands = fault === "early" ? openOnly.commands : retry;
        const result = await recordReplay(
          checker,
          { ...openOnly, commands },
          await startHandoff(t, fault),
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations.at(-1), {
            head: 2,
            receipt: { First: 1, Second: 2 },
            waiting: [],
            received: { First: 1, Second: null },
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, fault === "early" ? 1 : 6);
          assert.deepEqual(result.difference.command, {
            kind: fault === "early" ? "open" : "send",
            request: "First",
          });
          if (fault === "latest") {
            assert.deepEqual(result.difference.observed, {
              head: 2,
              receipt: { First: 1, Second: 2 },
              waiting: [],
              received: { First: 2, Second: null },
            });
          } else {
            assert.deepEqual(result.difference.observed, {
              head: 0,
              receipt: { First: null, Second: null },
              waiting: [],
              received: { First: 0, Second: null },
            });
          }
        }
      });
    }
  },
);
