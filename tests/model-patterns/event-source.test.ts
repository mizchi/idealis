import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import { chromium } from "@playwright/test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = {
  readonly kind: "connect" | "deliver" | "repeat" | "commit" | "disconnect";
};
type Harness = {
  start(): void;
  commit(): Promise<void>;
  pending: number | null;
  browserId: number;
  errors: number;
};
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/stream-terminal/event-source.model";

test(
  "Chromium auto-reconnect reports the event ID while delivery resumes from the application commit",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "BrowserAheadOfCommit",
    );
    assert.deepEqual(
      base.commands.map(({ kind }) => kind),
      ["connect", "deliver", "commit", "deliver"],
    );
    const commands: Command[] = [
      ...base.commands,
      { kind: "disconnect" },
      { kind: "connect" },
      { kind: "deliver" },
      { kind: "commit" },
      { kind: "repeat" },
      { kind: "commit" },
    ];
    const browser = await chromium.launch();
    t.after(() => browser.close());
    for (const fault of [null, "resume from header", "reapply repeat"]) {
      await t.test(
        fault ?? "resume from commit and ignore a repeated event",
        async (t) => {
          const db = new DatabaseSync(":memory:");
          db.exec("CREATE TABLE effects (position INTEGER NOT NULL)");
          const committed = () => {
            const row = db
              .prepare(
                "SELECT COALESCE(MAX(position), 0) AS cursor, COUNT(*) AS applied FROM effects",
              )
              .get();
            assert.ok(
              row &&
                typeof row.cursor === "number" &&
                typeof row.applied === "number",
            );
            return { cursor: row.cursor, applied: row.applied };
          };
          let queued:
            | { response: ServerResponse; header: number | null }
            | undefined;
          let active: ServerResponse | undefined;
          let header: number | null = null;
          let nextFrom = 0;
          const requests = new EventEmitter();
          const server = createServer((request, response) => {
            const url = new URL(request.url ?? "/", "http://localhost");
            if (url.pathname === "/events") {
              assert.equal(queued, undefined);
              const raw = request.headers["last-event-id"];
              assert.ok(raw === undefined || typeof raw === "string");
              queued = {
                response,
                header: raw === undefined ? null : Number(raw),
              };
              requests.emit("subscription");
            } else if (
              url.pathname === "/commit" &&
              request.method === "POST"
            ) {
              const position = Number(url.searchParams.get("position"));
              if (
                fault === "reapply repeat" ||
                position === committed().cursor + 1
              )
                db.prepare("INSERT INTO effects VALUES (?)").run(position);
              response.writeHead(204).end();
            } else if (url.pathname === "/") {
              response.writeHead(200, { "Content-Type": "text/html" }).end(`
            <!doctype html><script>
              window.harness = {
                pending: null, browserId: 0, errors: 0,
                start() {
                  const source = new EventSource("/events");
                  source.onmessage = (event) => {
                    this.browserId = Number(event.lastEventId);
                    this.pending = Number(event.data);
                  };
                  source.onerror = () => {
                    this.errors++;
                    this.pending = null;
                  };
                },
                async commit() {
                  const result = await fetch("/commit?position=" + this.pending, { method: "POST" });
                  if (!result.ok) throw new Error("commit failed");
                  this.pending = null;
                },
              };
            </script>
          `);
            } else response.writeHead(404).end();
          });
          server.listen(0, "127.0.0.1");
          await once(server, "listening");
          const page = await browser.newPage();
          t.after(async () => {
            await page.close();
            server.closeAllConnections();
            await new Promise<void>((resolve) => server.close(() => resolve()));
            db.close();
          });
          const address = server.address();
          assert.ok(address && typeof address !== "string");
          await page.goto(`http://127.0.0.1:${address.port}`);
          let started = false;
          const result = await recordReplay<Command>(
            checker,
            { ...base, commands },
            {
              async apply({ kind }) {
                switch (kind) {
                  case "connect":
                    if (!started) {
                      started = true;
                      await page.evaluate(() =>
                        (
                          window as unknown as { harness: Harness }
                        ).harness.start(),
                      );
                    }
                    if (!queued) await once(requests, "subscription");
                    assert.ok(queued);
                    ({ response: active, header } = queued);
                    queued = undefined;
                    nextFrom =
                      fault === "resume from header"
                        ? (header ?? 0)
                        : committed().cursor;
                    active.writeHead(200, {
                      "Content-Type": "text/event-stream",
                      "Cache-Control": "no-cache",
                    });
                    active.write("retry: 10\n\n");
                    break;
                  case "deliver":
                  case "repeat":
                    assert.ok(active);
                    if (kind === "deliver") nextFrom += 1;
                    active.write(`id: ${nextFrom}\ndata: ${nextFrom}\n\n`);
                    await page.waitForFunction(
                      (position) =>
                        (window as unknown as { harness: Harness }).harness
                          .pending === position,
                      nextFrom,
                    );
                    break;
                  case "commit":
                    await page.evaluate(() =>
                      (
                        window as unknown as { harness: Harness }
                      ).harness.commit(),
                    );
                    break;
                  case "disconnect":
                    assert.ok(active);
                    active.end();
                    active = undefined;
                    await page.waitForFunction(
                      () =>
                        (window as unknown as { harness: Harness }).harness
                          .errors === 1,
                    );
                    break;
                }
              },
              async observe() {
                const app = await page.evaluate(() => {
                  const { pending, browserId } = (
                    window as unknown as { harness: Harness }
                  ).harness;
                  return { pending, browserId };
                });
                return { ...app, ...committed(), header };
              },
              project: ({ inbox, browserId, cursor, applied, header }) => ({
                pending: inbox,
                browserId,
                cursor,
                applied,
                header,
              }),
            },
          );
          assert.deepEqual(result.recording.observations[6], {
            pending: null,
            browserId: 2,
            cursor: 1,
            applied: 1,
            header: 2,
          });
          if (fault === "resume from header") {
            assert.equal(result.verdict, "mismatch");
            assert.deepEqual(result.difference, {
              step: 7,
              command: { kind: "deliver" },
              expected: {
                pending: 2,
                browserId: 2,
                cursor: 1,
                applied: 1,
                header: 2,
              },
              observed: {
                pending: 3,
                browserId: 3,
                cursor: 1,
                applied: 1,
                header: 2,
              },
            });
          } else if (fault === "reapply repeat") {
            assert.equal(result.verdict, "mismatch");
            const expected = {
              pending: null,
              browserId: 2,
              cursor: 2,
              applied: 2,
              header: 2,
            };
            assert.deepEqual(result.difference, {
              step: 10,
              command: { kind: "commit" },
              expected,
              observed: { ...expected, applied: 3 },
            });
          } else {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(result.recording.observations.at(-1), {
              pending: null,
              browserId: 2,
              cursor: 2,
              applied: 2,
              header: 2,
            });
          }
        },
      );
    }
  },
);
