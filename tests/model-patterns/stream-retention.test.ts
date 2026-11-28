import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Client = "Fast" | "Slow";
type Command =
  | {
      readonly kind:
        | "acknowledge"
        | "reachDeadline"
        | "expire"
        | "reconnect"
        | "resnapshot";
      readonly client: Client;
    }
  | { readonly kind: "compact"; readonly through: number };
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/stream-terminal/retention.ideal";

test(
  "compaction protects live subscriber positions and forces a snapshot after an expired gap",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "SlowStillProtectsTheLog",
      { checker: "rprob" },
    );
    assert.equal(base.commands.length, 2);
    const commands: Command[] = [
      ...base.commands,
      { kind: "compact", through: 2 },
      { kind: "reachDeadline", client: "Slow" },
      { kind: "expire", client: "Slow" },
      { kind: "compact", through: 2 },
      { kind: "reconnect", client: "Slow" },
      { kind: "reconnect", client: "Fast" },
      { kind: "resnapshot", client: "Slow" },
      { kind: "reconnect", client: "Slow" },
    ];
    for (const fault of ["none", "use-max-cursor", "hide-gap"] as const) {
      await t.test(fault, async (t) => {
        const db = new DatabaseSync(":memory:");
        db.exec(`
        CREATE TABLE retention (floor INTEGER, head INTEGER);
        INSERT INTO retention VALUES (0, 2);
        CREATE TABLE log (position INTEGER PRIMARY KEY, payload TEXT);
        INSERT INTO log VALUES (1, 'one'), (2, 'two');
        CREATE TABLE clients (id TEXT PRIMARY KEY, cursor INTEGER, pinned INTEGER, due INTEGER);
        INSERT INTO clients VALUES ('Fast', 0, 1, 0), ('Slow', 0, 1, 0);
      `);
        const bounds = () => {
          const row = db.prepare("SELECT floor, head FROM retention").get();
          assert.ok(
            typeof row?.floor === "number" && typeof row.head === "number",
          );
          return { floor: row.floor, head: row.head };
        };
        const clientRow = (client: Client) => {
          const row = db
            .prepare("SELECT cursor, pinned FROM clients WHERE id = ?")
            .get(client);
          assert.ok(
            typeof row?.cursor === "number" && typeof row.pinned === "number",
          );
          return { cursor: row.cursor, pinned: row.pinned === 1 };
        };
        const server = createServer((request, response) => {
          const url = new URL(request.url ?? "/", "http://localhost");
          const client = url.searchParams.get("client");
          if (client !== "Fast" && client !== "Slow") {
            response.writeHead(404).end();
            return;
          }
          const checkpoint = clientRow(client).cursor;
          const { floor, head } = bounds();
          if (url.pathname === "/resume") {
            if (checkpoint < floor && fault !== "hide-gap") {
              response.writeHead(409).end("gap");
            } else {
              db.prepare(
                "UPDATE clients SET pinned = 1, due = 0 WHERE id = ?",
              ).run(client);
              response.writeHead(204).end();
            }
          } else if (
            url.pathname === "/snapshot" &&
            request.method === "POST"
          ) {
            db.prepare(
              "UPDATE clients SET cursor = ?, pinned = 1, due = 0 WHERE id = ?",
            ).run(head, client);
            response.end(JSON.stringify({ position: head }));
          } else response.writeHead(404).end();
        });
        server.listen(0, "127.0.0.1");
        await once(server, "listening");
        t.after(async () => {
          server.closeAllConnections();
          await new Promise<void>((resolve) => server.close(() => resolve()));
          db.close();
        });
        const address = server.address();
        assert.ok(address && typeof address !== "string");
        const url = `http://127.0.0.1:${address.port}`;
        const replies: Record<Client, "Gap" | "Ready" | null> = {
          Fast: null,
          Slow: null,
        };
        const result = await recordReplay<Command>(
          checker,
          { ...base, commands },
          {
            async apply(command) {
              switch (command.kind) {
                case "acknowledge": {
                  const { cursor, pinned } = clientRow(command.client);
                  assert.ok(
                    pinned &&
                      db
                        .prepare("SELECT 1 FROM log WHERE position = ?")
                        .get(cursor + 1),
                  );
                  db.prepare("UPDATE clients SET cursor = ? WHERE id = ?").run(
                    cursor + 1,
                    command.client,
                  );
                  break;
                }
                case "reachDeadline":
                  db.prepare(
                    "UPDATE clients SET due = 1 WHERE id = ? AND pinned = 1",
                  ).run(command.client);
                  break;
                case "expire":
                  db.prepare(
                    "UPDATE clients SET pinned = 0, due = 0 WHERE id = ? AND due = 1",
                  ).run(command.client);
                  break;
                case "compact": {
                  db.exec("BEGIN IMMEDIATE");
                  try {
                    const cutoff = db
                      .prepare(
                        fault === "use-max-cursor"
                          ? "SELECT MAX(cursor) AS position FROM clients WHERE pinned = 1"
                          : "SELECT MIN(cursor) AS position FROM clients WHERE pinned = 1",
                      )
                      .get()?.position;
                    if (
                      cutoff === null ||
                      (typeof cutoff === "number" && command.through <= cutoff)
                    ) {
                      db.prepare("UPDATE retention SET floor = ?").run(
                        command.through,
                      );
                      db.prepare("DELETE FROM log WHERE position <= ?").run(
                        command.through,
                      );
                    }
                    db.exec("COMMIT");
                  } catch (error) {
                    db.exec("ROLLBACK");
                    throw error;
                  }
                  break;
                }
                case "reconnect": {
                  const response = await fetch(
                    `${url}/resume?client=${command.client}`,
                  );
                  if (response.status === 409) {
                    assert.equal(await response.text(), "gap");
                    replies[command.client] = "Gap";
                  } else {
                    assert.equal(response.status, 204);
                    replies[command.client] = "Ready";
                  }
                  break;
                }
                case "resnapshot": {
                  const response = await fetch(
                    `${url}/snapshot?client=${command.client}`,
                    { method: "POST" },
                  );
                  assert.equal(response.status, 200);
                  assert.deepEqual(await response.json(), {
                    position: bounds().head,
                  });
                  replies[command.client] = "Ready";
                  break;
                }
              }
            },
            async observe() {
              const fast = clientRow("Fast");
              const slow = clientRow("Slow");
              return {
                floor: bounds().floor,
                retained: db
                  .prepare("SELECT position FROM log ORDER BY position")
                  .all()
                  .map(({ position }) => position),
                cursors: { Fast: fast.cursor, Slow: slow.cursor },
                protected: [
                  fast.pinned ? "Fast" : null,
                  slow.pinned ? "Slow" : null,
                ].filter((client) => client !== null),
                replies: { ...replies },
              };
            },
            project: ({ floor, cursors, protected: pinned, replies }) => ({
              floor,
              retained: [1, 2].filter((position) => position > Number(floor)),
              cursors,
              protected: pinned,
              replies,
            }),
          },
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[7], {
            floor: 2,
            retained: [],
            cursors: { Fast: 2, Slow: 0 },
            protected: ["Fast"],
            replies: { Fast: null, Slow: "Gap" },
          });
          assert.deepEqual(result.recording.observations.at(-1), {
            floor: 2,
            retained: [],
            cursors: { Fast: 2, Slow: 2 },
            protected: ["Fast", "Slow"],
            replies: { Fast: "Ready", Slow: "Ready" },
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(
            result.difference.step,
            fault === "use-max-cursor" ? 3 : 7,
          );
          assert.deepEqual(result.difference.observed, {
            floor: 2,
            retained: [],
            cursors: { Fast: 2, Slow: 0 },
            protected: ["Fast", "Slow"],
            replies: {
              Fast: null,
              Slow: fault === "hide-gap" ? "Ready" : null,
            },
          });
        }
      });
    }
  },
);
