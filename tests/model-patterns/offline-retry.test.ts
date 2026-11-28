import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer, type ServerResponse } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Request = "First" | "Second";
type Value = "Original" | "Remote" | "Local";
type Edit = { readonly baseValue: Value; readonly desired: Value };
type Reply = {
  readonly outcome: "Saved" | "Conflict" | "ReusedKey";
  readonly revision: number;
};
type Command =
  | { readonly kind: "submit"; readonly request: Request; readonly edit: Edit }
  | { readonly kind: "loseResponse" | "takeResponse" };
type Fault = "none" | "recompute" | "duplicate" | "reuse-result";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/offline-merge/retry.ideal";

test(
  "lost successful offline edits retain their original decision and revision",
  { timeout: 30_000 },
  async (t) => {
    const firstLost = await reachableScenario<Command>(
      checker,
      model,
      "FirstCommittedWithLostResponse",
      { checker: "rprob" },
    );
    const otherCommitted = await reachableScenario<Command>(
      checker,
      model,
      "OtherEditCommittedAfterLoss",
      { checker: "rprob" },
    );
    const conflictNowSucceeds = await reachableScenario<Command>(
      checker,
      model,
      "OriginalConflictWouldNowSucceed",
      { checker: "rprob" },
    );
    const edit = { baseValue: "Original", desired: "Remote" } as const;
    const cases = [
      {
        name: "after revision changes",
        base: otherCommitted,
        edit,
        fault: "recompute",
        expected: {
          live: "Local",
          revision: 2,
          lastReply: { outcome: "Saved", revision: 1 },
        },
        broken: {
          live: "Local",
          revision: 2,
          lastReply: { outcome: "Conflict", revision: 2 },
        },
      },
      {
        name: "original conflict after revision changes",
        base: conflictNowSucceeds,
        edit: { baseValue: "Remote", desired: "Local" },
        fault: "recompute",
        expected: {
          live: "Remote",
          revision: 1,
          lastReply: { outcome: "Conflict", revision: 0 },
        },
        broken: {
          live: "Remote",
          revision: 1,
          lastReply: { outcome: "Saved", revision: 2 },
        },
      },
      {
        name: "same live value",
        base: firstLost,
        edit,
        fault: "duplicate",
        expected: {
          live: "Remote",
          revision: 1,
          lastReply: { outcome: "Saved", revision: 1 },
        },
        broken: { live: "Remote", revision: 2, lastReply: null },
      },
      {
        name: "same key with another body",
        base: otherCommitted,
        edit: { ...edit, desired: "Local" },
        fault: "reuse-result",
        expected: {
          live: "Local",
          revision: 2,
          lastReply: { outcome: "ReusedKey", revision: 2 },
        },
        broken: {
          live: "Local",
          revision: 2,
          lastReply: { outcome: "Saved", revision: 1 },
        },
      },
    ] as const;
    for (const scenario of cases) {
      const faults: readonly Fault[] = ["none", scenario.fault];
      for (const fault of faults) {
        await t.test(`${scenario.name} / ${fault}`, async (t) => {
          const dir = await mkdtemp(join(tmpdir(), "idealis-offline-retry-"));
          const path = join(dir, "edits.sqlite");
          let db = new DatabaseSync(path);
          db.exec(`
          CREATE TABLE revisions (version INTEGER PRIMARY KEY, value TEXT NOT NULL);
          INSERT INTO revisions VALUES (0, 'Original');
          CREATE TABLE receipts (id TEXT PRIMARY KEY, base TEXT, desired TEXT, outcome TEXT, revision INTEGER);
        `);
          const load = () => {
            const row = db
              .prepare(
                "SELECT value, version FROM revisions ORDER BY version DESC LIMIT 1",
              )
              .get();
            assert.ok(row && typeof row.version === "number");
            return { live: row.value, revision: row.version };
          };
          const settle = (request: Request, edit: Edit): Reply => {
            db.exec("BEGIN IMMEDIATE");
            try {
              const saved = db
                .prepare(
                  "SELECT base, desired, outcome, revision FROM receipts WHERE id = ?",
                )
                .get(request);
              const current = load();
              let reply: Reply;
              if (
                saved &&
                (saved.base !== edit.baseValue ||
                  saved.desired !== edit.desired) &&
                fault !== "reuse-result"
              ) {
                reply = { outcome: "ReusedKey", revision: current.revision };
              } else if (
                saved &&
                fault !== "duplicate" &&
                fault !== "recompute"
              ) {
                assert.ok(
                  saved.outcome === "Saved" || saved.outcome === "Conflict",
                );
                assert.ok(typeof saved.revision === "number");
                reply = { outcome: saved.outcome, revision: saved.revision };
              } else {
                const conflict =
                  current.live !== edit.baseValue &&
                  current.live !== edit.desired;
                reply = {
                  outcome: conflict ? "Conflict" : "Saved",
                  revision: current.revision + (conflict ? 0 : 1),
                };
                if (!conflict && (!saved || fault === "duplicate")) {
                  db.prepare("INSERT INTO revisions VALUES (?, ?)").run(
                    reply.revision,
                    edit.desired,
                  );
                }
                if (!saved)
                  db.prepare("INSERT INTO receipts VALUES (?, ?, ?, ?, ?)").run(
                    request,
                    edit.baseValue,
                    edit.desired,
                    reply.outcome,
                    reply.revision,
                  );
              }
              db.exec("COMMIT");
              return reply;
            } catch (error) {
              db.exec("ROLLBACK");
              throw error;
            }
          };
          let held: { socket: ServerResponse; reply: Reply } | undefined;
          const committed = new EventEmitter();
          const server = createServer(async (request, response) => {
            try {
              let body = "";
              for await (const chunk of request) body += String(chunk);
              const { request: id, edit } = JSON.parse(body) as {
                request: Request;
                edit: Edit;
              };
              assert.equal(held, undefined);
              held = { socket: response, reply: settle(id, edit) };
              committed.emit("ready");
            } catch (error) {
              response.writeHead(500).end(String(error));
            }
          });
          server.listen(0, "127.0.0.1");
          await once(server, "listening");
          t.after(async () => {
            server.closeAllConnections();
            await new Promise<void>((resolve) => server.close(() => resolve()));
            db.close();
            await rm(dir, { recursive: true, force: true });
          });
          const address = server.address();
          assert.ok(address && typeof address !== "string");
          const url = `http://127.0.0.1:${address.port}`;
          let pending: Promise<Reply> | undefined;
          let lastReply: Reply | null = null;
          const commands: Command[] = [
            ...scenario.base.commands,
            { kind: "submit", request: "First", edit: scenario.edit },
            { kind: "takeResponse" },
          ];
          const result = await recordReplay<Command>(
            checker,
            { ...scenario.base, commands },
            {
              async apply(command) {
                if (command.kind === "submit") {
                  assert.equal(pending, undefined);
                  const ready = once(committed, "ready");
                  pending = fetch(url, {
                    method: "POST",
                    body: JSON.stringify(command),
                  }).then(async (response) => {
                    if (!response.ok) assert.fail(await response.text());
                    return (await response.json()) as Reply;
                  });
                  void pending.catch(() => {});
                  await Promise.race([ready, pending]);
                  return;
                }
                assert.ok(pending && held);
                if (command.kind === "loseResponse") {
                  held.socket.destroy();
                  await assert.rejects(pending, TypeError);
                  db.close();
                  db = new DatabaseSync(path);
                } else {
                  held.socket.end(JSON.stringify(held.reply));
                  lastReply = await pending;
                }
                held = undefined;
                pending = undefined;
              },
              async observe() {
                return { ...load(), lastReply };
              },
              project: ({ live, revision, lastReply }) => {
                const reply = lastReply as {
                  outcome: Reply["outcome"];
                  atRevision: number;
                } | null;
                return {
                  live,
                  revision,
                  lastReply:
                    reply === null
                      ? null
                      : { outcome: reply.outcome, revision: reply.atRevision },
                };
              },
            },
          );

          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(
              result.recording.observations.at(-1),
              scenario.expected,
            );
          } else {
            assert.equal(result.verdict, "mismatch");
            assert.equal(
              result.difference.step,
              commands.length - (fault === "duplicate" ? 1 : 0),
            );
            assert.deepEqual(result.difference.observed, scenario.broken);
          }
        });
      }
    }
  },
);
