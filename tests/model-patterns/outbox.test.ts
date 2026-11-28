import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  replay,
  type Scenario,
} from "../../examples/model-patterns/replay.js";

type Command =
  | {
      readonly kind: "commit" | "send" | "finalize";
      readonly request: "First" | "Second";
    }
  | { readonly kind: "crash" | "restart" };
type Fault = "none" | "skip-outbox" | "repeat-effect";
const model = "examples/model-patterns/outbox/delivery.model";
const rcheck = "crates/rcheck/target/debug/rcheck";

test(
  "an outbox survives both send boundaries and the receiver applies a retry once",
  { timeout: 30_000 },
  async () => {
    const dir = await mkdtemp(join(tmpdir(), "idealis-outbox-"));
    try {
      for (const sentBeforeCrash of [false, true]) {
        const trace = await reachableScenario<Command>(
          rcheck,
          model,
          sentBeforeCrash ? "StoppedAfterSend" : "StoppedBeforeSend",
        );
        const fault: Fault = sentBeforeCrash ? "repeat-effect" : "skip-outbox";
        for (const mode of ["none", fault] as const) {
          const name = `${sentBeforeCrash}-${mode}`;
          const senderPath = join(dir, `${name}-sender.sqlite`);
          let sender: DatabaseSync | null = new DatabaseSync(senderPath);
          sender.exec(
            "CREATE TABLE committed (id TEXT PRIMARY KEY); CREATE TABLE outbox (id TEXT PRIMARY KEY)",
          );
          const receiver = new DatabaseSync(
            join(dir, `${name}-receiver.sqlite`),
          );
          receiver.exec(
            "CREATE TABLE receipts (id TEXT PRIMARY KEY); CREATE TABLE effects (id TEXT NOT NULL); CREATE TABLE attempts (id TEXT NOT NULL)",
          );
          const received = createServer((request, response) => {
            const id = request.url?.slice(1);
            if (
              request.method !== "POST" ||
              (id !== "First" && id !== "Second")
            ) {
              response.writeHead(400).end();
              return;
            }
            try {
              receiver.exec("BEGIN");
              receiver.prepare("INSERT INTO attempts VALUES (?)").run(id);
              const first = receiver
                .prepare("INSERT OR IGNORE INTO receipts VALUES (?)")
                .run(id);
              if (Number(first.changes) === 1 || mode === "repeat-effect") {
                receiver.prepare("INSERT INTO effects VALUES (?)").run(id);
              }
              receiver.exec("COMMIT");
              response.writeHead(204).end();
            } catch {
              receiver.exec("ROLLBACK");
              response.writeHead(500).end();
            }
          });
          try {
            await new Promise<void>((resolve, reject) => {
              received.once("error", reject);
              received.listen(0, "127.0.0.1", resolve);
            });
            const address = received.address();
            assert.ok(address && typeof address !== "string");
            const origin = `http://127.0.0.1:${address.port}`;
            const sent = new Set<string>();
            const commands: Command[] = [
              ...trace.commands,
              { kind: "restart" },
              { kind: "send", request: "First" },
              { kind: "finalize", request: "First" },
              { kind: "commit", request: "Second" },
              { kind: "send", request: "Second" },
              { kind: "finalize", request: "Second" },
            ];
            const scenario: Scenario<Command> = {
              model,
              config: trace.config,
              initial: trace.initial,
              commands,
            };
            const run = replay(rcheck, scenario, {
              async apply(command) {
                switch (command.kind) {
                  case "crash":
                    sender?.close();
                    sender = null;
                    sent.clear();
                    break;
                  case "restart":
                    sender = new DatabaseSync(senderPath);
                    break;
                  case "commit":
                    assert.ok(sender);
                    sender.exec("BEGIN");
                    try {
                      sender
                        .prepare("INSERT INTO committed VALUES (?)")
                        .run(command.request);
                      if (mode !== "skip-outbox") {
                        sender
                          .prepare("INSERT INTO outbox VALUES (?)")
                          .run(command.request);
                      }
                      sender.exec("COMMIT");
                    } catch (error) {
                      sender.exec("ROLLBACK");
                      throw error;
                    }
                    break;
                  case "send": {
                    assert.ok(
                      sender
                        ?.prepare("SELECT id FROM outbox WHERE id = ?")
                        .get(command.request),
                    );
                    const reply = await fetch(`${origin}/${command.request}`, {
                      method: "POST",
                      signal: AbortSignal.timeout(5_000),
                    });
                    assert.equal(reply.status, 204);
                    sent.add(command.request);
                    break;
                  }
                  case "finalize":
                    assert.ok(sent.delete(command.request));
                    assert.equal(
                      Number(
                        sender
                          ?.prepare("DELETE FROM outbox WHERE id = ?")
                          .run(command.request).changes,
                      ),
                      1,
                    );
                    break;
                }
              },
              async observe() {
                const snapshot = new DatabaseSync(senderPath, {
                  readOnly: true,
                });
                try {
                  return {
                    committed: snapshot
                      .prepare("SELECT id FROM committed ORDER BY id")
                      .all()
                      .map((row) => row.id),
                    pending: snapshot
                      .prepare("SELECT id FROM outbox ORDER BY id")
                      .all()
                      .map((row) => row.id),
                    sent: [...sent].sort(),
                    delivered: receiver
                      .prepare("SELECT DISTINCT id FROM effects ORDER BY id")
                      .all()
                      .map((row) => row.id),
                    duplicated: receiver
                      .prepare(
                        "SELECT id FROM effects GROUP BY id HAVING COUNT(*) > 1 ORDER BY id",
                      )
                      .all()
                      .map((row) => row.id),
                    offline: sender === null,
                  };
                } finally {
                  snapshot.close();
                }
              },
              project(state) {
                return Object.fromEntries(
                  Object.entries(state).map(([key, value]) => [
                    key,
                    Array.isArray(value) ? [...value].sort() : value,
                  ]),
                );
              },
            });
            if (mode === "skip-outbox") {
              await assert.rejects(
                run,
                /"step":1.*"observed":\{"committed":\["First"\],"pending":\[\]/,
              );
            } else if (mode === "repeat-effect") {
              await assert.rejects(
                run,
                /"step":5.*"observed":.*"duplicated":\["First"\]/,
              );
            } else {
              await run;
              for (const [id, expected] of [
                ["First", sentBeforeCrash ? 2 : 1],
                ["Second", 1],
              ] as const) {
                assert.equal(
                  receiver
                    .prepare("SELECT COUNT(*) AS n FROM attempts WHERE id = ?")
                    .get(id)?.n,
                  expected,
                );
                assert.equal(
                  receiver
                    .prepare("SELECT COUNT(*) AS n FROM effects WHERE id = ?")
                    .get(id)?.n,
                  1,
                );
              }
            }
          } finally {
            if (received.listening) {
              await new Promise<void>((resolve, reject) =>
                received.close((error) => (error ? reject(error) : resolve())),
              );
            }
            sender?.close();
            receiver.close();
          }
        }
      }
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  },
);
