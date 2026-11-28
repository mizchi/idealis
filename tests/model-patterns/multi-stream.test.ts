import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";
import { readPositionStream } from "./helpers/position-stream.js";

type Client = "Fast" | "Slow";
type Command = {
  readonly kind: "connect" | "deliver" | "commit" | "disconnect";
  readonly client: Client;
};
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/stream-terminal/multi-stream.model";
const log = [1, 2, 3] as const;

test(
  "subscribers reconnect to the same log using their own committed offsets",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "DifferentProgressBeforeReconnect",
    );
    for (const client of ["Fast", "Slow"]) {
      assert.equal(
        base.commands.filter(
          (command) => command.kind === "connect" && command.client === client,
        ).length,
        1,
      );
    }
    const commands: Command[] = [
      ...base.commands,
      { kind: "connect", client: "Fast" },
      { kind: "connect", client: "Slow" },
      { kind: "deliver", client: "Slow" },
      { kind: "deliver", client: "Fast" },
      { kind: "commit", client: "Fast" },
      { kind: "commit", client: "Slow" },
    ];
    for (const fault of [
      "none",
      "resume-at-global-max",
      "commit-all-cursors",
    ] as const) {
      await t.test(fault, async (t) => {
        const db = new DatabaseSync(":memory:");
        db.exec(`
        CREATE TABLE subscriptions (id TEXT PRIMARY KEY, cursor INTEGER NOT NULL);
        INSERT INTO subscriptions VALUES ('Fast', 0), ('Slow', 0);
        CREATE TABLE effects (id TEXT NOT NULL, position INTEGER NOT NULL, PRIMARY KEY (id, position));
      `);
        const cursorFor = (client: Client) => {
          const row = db
            .prepare("SELECT cursor FROM subscriptions WHERE id = ?")
            .get(client);
          assert.ok(typeof row?.cursor === "number");
          return row.cursor;
        };
        const streams = new Map<
          Client,
          { response: ServerResponse; position: number }
        >();
        const opened = new Set<Client>();
        const server = createServer((request, response) => {
          const client = new URL(
            request.url ?? "/",
            "http://localhost",
          ).searchParams.get("subscriber");
          if (client !== "Fast" && client !== "Slow") {
            response.writeHead(404).end();
            return;
          }
          const position =
            fault === "resume-at-global-max" && opened.has(client)
              ? Math.max(cursorFor("Fast"), cursorFor("Slow"))
              : cursorFor(client);
          streams.set(client, { response, position });
          opened.add(client);
          response.writeHead(200, { "Content-Type": "application/x-ndjson" });
          response.flushHeaders();
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
        const url = `http://127.0.0.1:${address.port}/events`;
        const receivers = new Map<
          Client,
          ReturnType<typeof readPositionStream>
        >();
        const pending = new Map<Client, number>();
        const result = await recordReplay<Command>(
          checker,
          { ...base, commands },
          {
            async apply({ kind, client }) {
              switch (kind) {
                case "connect": {
                  const response = await fetch(`${url}?subscriber=${client}`);
                  assert.equal(response.status, 200);
                  assert.ok(response.body);
                  receivers.set(client, readPositionStream(response.body));
                  break;
                }
                case "deliver": {
                  const stream = streams.get(client);
                  const receiver = receivers.get(client);
                  assert.ok(stream && receiver);
                  const position = log[stream.position];
                  assert.ok(position !== undefined);
                  stream.position = position;
                  stream.response.write(JSON.stringify({ position }) + "\n");
                  pending.set(client, await receiver.next());
                  break;
                }
                case "commit": {
                  const position = pending.get(client);
                  assert.ok(position !== undefined);
                  pending.delete(client);
                  if (position !== cursorFor(client) + 1) break;
                  db.exec("BEGIN IMMEDIATE");
                  try {
                    db.prepare("INSERT INTO effects VALUES (?, ?)").run(
                      client,
                      position,
                    );
                    if (fault === "commit-all-cursors")
                      db.prepare("UPDATE subscriptions SET cursor = ?").run(
                        position,
                      );
                    else
                      db.prepare(
                        "UPDATE subscriptions SET cursor = ? WHERE id = ?",
                      ).run(position, client);
                    db.exec("COMMIT");
                  } catch (error) {
                    db.exec("ROLLBACK");
                    throw error;
                  }
                  break;
                }
                case "disconnect": {
                  const stream = streams.get(client);
                  const receiver = receivers.get(client);
                  assert.ok(stream && receiver);
                  const closed = once(stream.response, "close");
                  stream.response.destroy();
                  await closed;
                  await receiver.cancelAfterDisconnect();
                  streams.delete(client);
                  receivers.delete(client);
                  pending.delete(client);
                  break;
                }
              }
            },
            async observe() {
              const count = db.prepare(
                "SELECT COUNT(*) AS n FROM effects WHERE id = ?",
              );
              return {
                cursors: { Fast: cursorFor("Fast"), Slow: cursorFor("Slow") },
                inbox: {
                  Fast: pending.get("Fast") ?? null,
                  Slow: pending.get("Slow") ?? null,
                },
                applied: {
                  Fast: count.get("Fast")?.n,
                  Slow: count.get("Slow")?.n,
                },
              };
            },
            project: ({ cursors, inbox, applied }) => ({
              cursors,
              inbox,
              applied,
            }),
          },
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations.at(-1), {
            cursors: { Fast: 3, Slow: 2 },
            inbox: { Fast: null, Slow: null },
            applied: { Fast: 3, Slow: 2 },
          });
        } else if (fault === "resume-at-global-max") {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, base.commands.length + 3);
          assert.deepEqual(result.difference.observed, {
            cursors: { Fast: 2, Slow: 1 },
            inbox: { Fast: null, Slow: 3 },
            applied: { Fast: 2, Slow: 1 },
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.command?.kind, "commit");
          assert.deepEqual(
            (result.difference.observed as { cursors: unknown }).cursors,
            { Fast: 1, Slow: 1 },
          );
        }
      });
    }
  },
);
