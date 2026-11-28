import assert from "node:assert/strict";
import { once } from "node:events";
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
import { readPositionStream } from "./helpers/position-stream.js";

type Command = {
  readonly kind:
    | "connect"
    | "send"
    | "replayBoundary"
    | "receive"
    | "commit"
    | "disconnect";
};
type Fault = "none" | "resume-from-sent" | "duplicate-boundary";
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/stream-terminal/resume.ideal";

test(
  "HTTP streams resume from the durable offset across two disconnect windows",
  { timeout: 30_000 },
  async (t) => {
    const unread = await reachableScenario<Command>(
      checker,
      model,
      "SentButUnread",
    );
    const uncommitted = await reachableScenario<Command>(
      checker,
      model,
      "ReadButUncommitted",
    );
    const cases = [
      {
        name: "sent but unread",
        base: unread,
        replay: false,
        fault: "resume-from-sent",
      },
      {
        name: "read but uncommitted",
        base: uncommitted,
        replay: false,
        fault: "resume-from-sent",
      },
      {
        name: "duplicate at the boundary",
        base: uncommitted,
        replay: true,
        fault: "duplicate-boundary",
      },
    ] as const;
    for (const scenario of cases) {
      const faults: readonly Fault[] = ["none", scenario.fault];
      for (const fault of faults) {
        await t.test(`${scenario.name} / ${fault}`, async (t) => {
          const dir = await mkdtemp(join(tmpdir(), "idealis-stream-resume-"));
          const path = join(dir, "client.sqlite");
          let db = new DatabaseSync(path);
          db.exec(
            "CREATE TABLE cursor (position INTEGER); INSERT INTO cursor VALUES (0); CREATE TABLE effects (position INTEGER)",
          );
          const cursor = () => {
            const row = db.prepare("SELECT position FROM cursor").get();
            assert.ok(typeof row?.position === "number");
            return row.position;
          };
          let writer: ServerResponse | undefined;
          let nextFrom = 0;
          let lastSent = 0;
          const server = createServer((request, response) => {
            assert.equal(writer, undefined);
            const url = new URL(request.url ?? "/", "http://localhost");
            nextFrom = Number(url.searchParams.get("after"));
            writer = response;
            response.writeHead(200, { "Content-Type": "application/x-ndjson" });
            response.flushHeaders();
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
          const origin = `http://127.0.0.1:${address.port}`;
          let reader: ReturnType<typeof readPositionStream> | undefined;
          let inbox: number | null = null;
          const commands: Command[] = [
            ...scenario.base.commands,
            { kind: "disconnect" },
            { kind: "connect" },
          ];
          if (scenario.replay)
            commands.push(
              { kind: "replayBoundary" },
              { kind: "receive" },
              { kind: "commit" },
            );
          commands.push(
            { kind: "send" },
            { kind: "receive" },
            { kind: "commit" },
          );
          const result = await recordReplay<Command>(
            checker,
            { ...scenario.base, commands },
            {
              async apply({ kind }) {
                switch (kind) {
                  case "connect": {
                    const from =
                      fault === "resume-from-sent" ? lastSent : cursor();
                    const response = await fetch(
                      `${origin}/events?after=${from}`,
                    );
                    assert.equal(response.status, 200);
                    assert.ok(response.body);
                    reader = readPositionStream(response.body);
                    break;
                  }
                  case "send":
                    assert.ok(writer);
                    lastSent = ++nextFrom;
                    writer.write(JSON.stringify({ position: lastSent }) + "\n");
                    break;
                  case "replayBoundary":
                    assert.ok(writer);
                    writer.write(JSON.stringify({ position: nextFrom }) + "\n");
                    break;
                  case "receive": {
                    assert.ok(reader);
                    inbox = await reader.next();
                    break;
                  }
                  case "commit": {
                    assert.ok(inbox !== null);
                    const position = cursor();
                    if (
                      inbox === position + 1 ||
                      (fault === "duplicate-boundary" && inbox === position)
                    ) {
                      db.exec("BEGIN IMMEDIATE");
                      try {
                        db.prepare("INSERT INTO effects VALUES (?)").run(inbox);
                        db.prepare("UPDATE cursor SET position = ?").run(inbox);
                        db.exec("COMMIT");
                      } catch (error) {
                        db.exec("ROLLBACK");
                        throw error;
                      }
                    }
                    inbox = null;
                    break;
                  }
                  case "disconnect": {
                    assert.ok(writer && reader);
                    const closed = once(writer, "close");
                    writer.destroy();
                    await closed;
                    await reader.cancelAfterDisconnect();
                    reader = undefined;
                    writer = undefined;
                    inbox = null;
                    db.close();
                    db = new DatabaseSync(path);
                    break;
                  }
                }
              },
              async observe() {
                return {
                  connected: reader !== undefined,
                  cursor: cursor(),
                  applied: db
                    .prepare("SELECT COUNT(*) AS total FROM effects")
                    .get()?.total,
                  inbox,
                };
              },
              project: ({ online, cursor, applied, inbox }) => ({
                connected: online,
                cursor,
                applied,
                inbox,
              }),
            },
          );
          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(result.recording.observations.at(-1), {
              connected: true,
              cursor: 2,
              applied: 2,
              inbox: null,
            });
          } else {
            assert.equal(result.verdict, "mismatch");
            assert.equal(
              result.difference.step,
              scenario.base.commands.length + (scenario.replay ? 5 : 4),
            );
            assert.equal(
              result.difference.command?.kind,
              scenario.replay ? "commit" : "receive",
            );
            assert.deepEqual(
              result.difference.observed,
              fault === "resume-from-sent"
                ? { connected: true, cursor: 1, applied: 1, inbox: 3 }
                : { connected: true, cursor: 1, applied: 2, inbox: null },
            );
          }
        });
      }
    }
  },
);
