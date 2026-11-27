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

type Command = {
  readonly kind:
    | "connect"
    | "scan"
    | "progress"
    | "commit"
    | "compact"
    | "disconnect";
};
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/stream-terminal/filter-progress.model";

test(
  "filtered stream progress never advances past an unapplied matching event",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "FirstGapScanned",
    );
    assert.deepEqual(
      base.commands.map(({ kind }) => kind),
      ["connect", "scan"],
    );
    const commands: Command[] = [
      ...base.commands,
      { kind: "progress" },
      { kind: "compact" },
      { kind: "scan" },
      { kind: "scan" },
      { kind: "progress" },
      { kind: "disconnect" },
      { kind: "connect" },
      { kind: "scan" },
      { kind: "commit" },
      { kind: "scan" },
      { kind: "progress" },
      { kind: "compact" },
      { kind: "scan" },
      { kind: "commit" },
    ];
    for (const fault of [
      "none",
      "ignore-progress",
      "use-scanned-head",
    ] as const) {
      await t.test(fault, async (t) => {
        const db = new DatabaseSync(":memory:");
        db.exec(`
        CREATE TABLE log (position INTEGER PRIMARY KEY, wanted INTEGER);
        INSERT INTO log VALUES (1, 0), (2, 1), (3, 0), (4, 1);
        CREATE TABLE state (cursor INTEGER, floor INTEGER);
        INSERT INTO state VALUES (0, 0);
        CREATE TABLE effects (position INTEGER PRIMARY KEY);
      `);
        const checkpoint = () => {
          const row = db.prepare("SELECT cursor, floor FROM state").get();
          assert.ok(
            typeof row?.cursor === "number" && typeof row.floor === "number",
          );
          return { cursor: row.cursor, floor: row.floor };
        };
        let scanned = 0;
        let serverPending: number | undefined;
        let writer: ServerResponse | undefined;
        let reader: ReturnType<typeof readPositionStream> | undefined;
        let pending: number | null = null;
        const server = createServer((request, response) => {
          const pathname = new URL(request.url ?? "/", "http://localhost")
            .pathname;
          if (pathname === "/events") {
            assert.equal(writer, undefined);
            writer = response;
            scanned = checkpoint().cursor;
            response.writeHead(200, { "Content-Type": "application/x-ndjson" });
            response.flushHeaders();
          } else if (pathname === "/progress") {
            const position =
              serverPending === undefined || fault === "use-scanned-head"
                ? scanned
                : serverPending - 1;
            response.end(JSON.stringify({ position }));
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
        const adapter = {
          async apply({ kind }: Command) {
            switch (kind) {
              case "connect": {
                const response = await fetch(`${url}/events`);
                assert.equal(response.status, 200);
                assert.ok(response.body);
                reader = readPositionStream(response.body);
                break;
              }
              case "scan": {
                assert.ok(reader && writer);
                const row = db
                  .prepare(
                    "SELECT position, wanted FROM log WHERE position > ? ORDER BY position LIMIT 1",
                  )
                  .get(scanned);
                assert.ok(typeof row?.position === "number");
                scanned = row.position;
                if (row.wanted === 1) {
                  assert.equal(serverPending, undefined);
                  serverPending = row.position;
                  writer.write(
                    JSON.stringify({ position: row.position }) + "\n",
                  );
                  pending = await reader.next();
                }
                break;
              }
              case "progress": {
                const response = await fetch(`${url}/progress`);
                assert.equal(response.status, 200);
                const message = (await response.json()) as { position: number };
                if (fault !== "ignore-progress")
                  db.prepare("UPDATE state SET cursor = ?").run(
                    message.position,
                  );
                break;
              }
              case "commit":
                assert.ok(pending !== null);
                db.exec("BEGIN IMMEDIATE");
                try {
                  db.prepare("INSERT INTO effects VALUES (?)").run(pending);
                  db.prepare("UPDATE state SET cursor = ?").run(pending);
                  db.exec("COMMIT");
                } catch (error) {
                  db.exec("ROLLBACK");
                  throw error;
                }
                pending = null;
                serverPending = undefined;
                break;
              case "compact": {
                const through = checkpoint().cursor;
                db.exec("BEGIN IMMEDIATE");
                try {
                  db.prepare("DELETE FROM log WHERE position <= ?").run(
                    through,
                  );
                  db.prepare("UPDATE state SET floor = ?").run(through);
                  db.exec("COMMIT");
                } catch (error) {
                  db.exec("ROLLBACK");
                  throw error;
                }
                break;
              }
              case "disconnect": {
                assert.ok(reader && writer);
                const closed = once(writer, "close");
                writer.destroy();
                await closed;
                await reader.cancelAfterDisconnect();
                reader = undefined;
                writer = undefined;
                pending = null;
                serverPending = undefined;
                break;
              }
            }
          },
          async observe() {
            return {
              ...checkpoint(),
              pending,
              applied: db
                .prepare("SELECT position FROM effects ORDER BY position")
                .all()
                .map(({ position }) => position),
            };
          },
          project: ({
            cursor,
            floor,
            pending,
            applied,
          }: Readonly<Record<string, unknown>>) => ({
            cursor,
            floor,
            pending,
            applied: (applied as string[]).map((row) => Number(row.slice(1))),
          }),
        };
        const result = await recordReplay<Command>(
          checker,
          { ...base, commands },
          adapter,
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[4], {
            cursor: 1,
            floor: 1,
            pending: null,
            applied: [],
          });
          assert.deepEqual(result.recording.observations[7], {
            cursor: 1,
            floor: 1,
            pending: 2,
            applied: [],
          });
          assert.deepEqual(result.recording.observations.at(-1), {
            cursor: 4,
            floor: 3,
            pending: null,
            applied: [2, 4],
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(
            result.difference.step,
            fault === "ignore-progress" ? 3 : 7,
          );
          assert.deepEqual(
            result.difference.observed,
            fault === "ignore-progress"
              ? { cursor: 0, floor: 0, pending: null, applied: [] }
              : { cursor: 3, floor: 1, pending: 2, applied: [] },
          );
          if (fault === "use-scanned-head") {
            await adapter.apply({ kind: "disconnect" });
            await adapter.apply({ kind: "connect" });
            await adapter.apply({ kind: "scan" });
            assert.deepEqual(await adapter.observe(), {
              cursor: 3,
              floor: 1,
              pending: 4,
              applied: [],
            });
          }
        }
      });
    }
  },
);
