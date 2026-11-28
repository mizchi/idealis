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

type Item = "First" | "Second";
type Response = { state: "Partial" | "Complete"; items: Item[] };
type Command =
  | { readonly kind: "apply"; readonly item: Item }
  | { readonly kind: "start" | "disconnect" | "finish" };
const items: readonly Item[] = ["First", "Second"];
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/offline-merge/batch-retry.ideal";

test(
  "batch retry resumes an HTTP request after only one SQLite item committed",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "FirstCommittedBeforeDisconnect",
    );
    assert.deepEqual(base.commands, [
      { kind: "start" },
      { kind: "apply", item: "First" },
      { kind: "disconnect" },
    ]);
    const commands: Command[] = [
      ...base.commands,
      { kind: "start" },
      { kind: "apply", item: "First" },
      { kind: "finish" },
      { kind: "start" },
      { kind: "apply", item: "First" },
      { kind: "apply", item: "Second" },
      { kind: "finish" },
    ];
    const faults = [
      { mode: "none" },
      {
        mode: "duplicate-item",
        step: 5,
        observed: { counts: { First: 2, Second: 0 }, response: null },
      },
      {
        mode: "report-complete-early",
        step: 6,
        observed: {
          counts: { First: 1, Second: 0 },
          response: { state: "Complete", items: ["First"] },
        },
      },
      {
        mode: "skip-whole-batch",
        step: 9,
        observed: { counts: { First: 1, Second: 0 }, response: null },
      },
    ] as const;
    for (const fault of faults) {
      await t.test(fault.mode, async (t) => {
        const dir = await mkdtemp(join(tmpdir(), "idealis-batch-retry-"));
        const path = join(dir, "batch.sqlite");
        let db = new DatabaseSync(path);
        db.exec(`
        CREATE TABLE receipts (item TEXT PRIMARY KEY);
        CREATE TABLE effects (item TEXT NOT NULL);
      `);
        const committed = () =>
          db
            .prepare("SELECT item FROM receipts ORDER BY item")
            .all()
            .map(({ item }) => item as Item);
        const opened = new EventEmitter();
        let held: ServerResponse | undefined;
        let continuing = false;
        const server = createServer(async (request, response) => {
          try {
            let body = "";
            for await (const chunk of request) body += String(chunk);
            assert.deepEqual(JSON.parse(body), items);
            assert.equal(held, undefined);
            held = response;
            continuing = committed().length > 0;
            opened.emit("opened");
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
        let pending: Promise<Response> | undefined;
        let response: Response | null = null;
        const result = await recordReplay<Command>(
          checker,
          { ...base, commands },
          {
            async apply(command) {
              switch (command.kind) {
                case "start": {
                  response = null;
                  const ready = once(opened, "opened");
                  pending = fetch(url, {
                    method: "POST",
                    body: JSON.stringify(items),
                  }).then(async (reply) => {
                    if (!reply.ok) assert.fail(await reply.text());
                    return (await reply.json()) as Response;
                  });
                  void pending.catch(() => {});
                  await Promise.race([ready, pending]);
                  break;
                }
                case "apply":
                  assert.ok(held);
                  if (continuing && fault.mode === "skip-whole-batch") break;
                  db.exec("BEGIN IMMEDIATE");
                  try {
                    const inserted = db
                      .prepare("INSERT OR IGNORE INTO receipts VALUES (?)")
                      .run(command.item);
                    if (
                      Number(inserted.changes) === 1 ||
                      fault.mode === "duplicate-item"
                    ) {
                      db.prepare("INSERT INTO effects VALUES (?)").run(
                        command.item,
                      );
                    }
                    db.exec("COMMIT");
                  } catch (error) {
                    db.exec("ROLLBACK");
                    throw error;
                  }
                  break;
                case "disconnect":
                  assert.ok(held && pending);
                  held.destroy();
                  await assert.rejects(pending, TypeError);
                  held = undefined;
                  pending = undefined;
                  db.close();
                  db = new DatabaseSync(path);
                  break;
                case "finish": {
                  assert.ok(held && pending);
                  const done = committed();
                  held.end(
                    JSON.stringify({
                      state:
                        done.length === items.length ||
                        (done.length > 0 &&
                          fault.mode === "report-complete-early")
                          ? "Complete"
                          : "Partial",
                      items: done,
                    }),
                  );
                  response = await pending;
                  held = undefined;
                  pending = undefined;
                  break;
                }
              }
            },
            async observe() {
              const count = db.prepare(
                "SELECT COUNT(*) AS total FROM effects WHERE item = ?",
              );
              return {
                counts: {
                  First: count.get("First")?.total,
                  Second: count.get("Second")?.total,
                },
                response,
              };
            },
            project: ({ effects, sent, responseItems }) => {
              const marked = effects as Record<Item, "Once" | "Twice" | null>;
              const count = (item: Item) =>
                marked[item] === "Once" ? 1 : marked[item] === "Twice" ? 2 : 0;
              return {
                counts: { First: count("First"), Second: count("Second") },
                response:
                  sent === null ? null : { state: sent, items: responseItems },
              };
            },
          },
        );
        if (fault.mode === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[6], {
            counts: { First: 1, Second: 0 },
            response: { state: "Partial", items: ["First"] },
          });
          assert.deepEqual(result.recording.observations.at(-1), {
            counts: { First: 1, Second: 1 },
            response: { state: "Complete", items: ["First", "Second"] },
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, fault.step);
          assert.deepEqual(result.difference.observed, fault.observed);
        }
      });
    }
  },
);
