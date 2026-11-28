import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { test } from "node:test";
import { Miniflare } from "miniflare";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  replayRecording,
} from "../../examples/model-patterns/replay.js";
import type {
  Database,
  Observation,
} from "../../examples/model-patterns/worker-microblog/worker.js";

const directory = "examples/model-patterns/worker-microblog";
const checker = "crates/idealis-check/target/debug/idealis-check";

// Project what the runtime exposes. Queue internals are not read from the model.
async function snapshot(db: Database) {
  const [posts, follows, feed, applied] = await Promise.all([
    db
      .prepare("SELECT post, author FROM posts ORDER BY post")
      .all<{ post: string; author: string }>(),
    db
      .prepare("SELECT reader, author FROM follows ORDER BY author")
      .all<{ reader: string; author: string }>(),
    db
      .prepare("SELECT reader, post FROM feed ORDER BY post")
      .all<{ reader: string; post: string }>(),
    db
      .prepare("SELECT post FROM applied ORDER BY post")
      .all<{ post: string }>(),
  ]);
  const users = ["Author", "Reader"];
  return {
    authors: Object.fromEntries(
      ["Earlier", "Later"].map((p) => [
        p,
        posts.results.find((row) => row.post === p)?.author ?? null,
      ]),
    ),
    follows: Object.fromEntries(
      users.map((u) => [
        u,
        follows.results
          .filter((row) => row.reader === u)
          .map((row) => row.author),
      ]),
    ),
    entries: Object.fromEntries(
      users.map((u) => [
        u,
        feed.results.filter((row) => row.reader === u).map((row) => row.post),
      ]),
    ),
    applied: applied.results.map((row) => row.post),
  };
}

test(
  "Workers Queue redelivers after a committed D1 write and the observed trace matches Idealis",
  { timeout: 20000 },
  async (t) => {
    const { model, config, initial } = await reachableScenario<
      Observation["command"]
    >(checker, `${directory}/model.project.json`, "RedeliveredAfterWrite");
    const progress = new EventEmitter();
    const enqueued = once(progress, "queue_enqueue");
    const acked = once(progress, "queue_ack");
    const events: Observation[] = [];
    const observations: Awaited<ReturnType<typeof snapshot>>[] = [];
    let db: Database;

    const modulesRoot = resolve("dist", directory);
    const mf = new Miniflare({
      compatibilityDate: "2026-07-30",
      modulesRoot,
      modules: ["worker.js", "app.js"].map((file) => ({
        type: "ESModule",
        path: resolve(modulesRoot, file),
      })),
      queueProducers: { POSTS: "microblog" },
      queueConsumers: {
        microblog: { maxBatchSize: 5, maxBatchTimeout: 1, maxRetries: 1 },
      },
      d1Databases: ["DB"],
      serviceBindings: {
        OBSERVE: async (request: { json(): Promise<unknown> }) => {
          const event = (await request.json()) as Observation;
          // If a callback arrives first, wait until send() completion has been observed.
          if (event.command.kind === "queue_deliver") await enqueued;
          const state = await snapshot(db);
          const firstApply =
            event.command.kind === "timeline_apply" &&
            !events.some(({ command }) => command.kind === "timeline_apply");
          events.push(event);
          observations.push(state);
          progress.emit(event.command.kind);
          // Interrupt the consumer after the app commit and before it requests ACK.
          return new Response(null, { status: firstApply ? 503 : 204 });
        },
      },
    });
    t.after(() => mf.dispose());

    db = await mf.getD1Database("DB");
    const schema = await readFile(`${directory}/schema.sql`, "utf8");
    await db.batch(
      schema
        .split(";")
        .filter((statement) => statement.trim())
        .map((statement) => db.prepare(statement)),
    );
    observations.push(await snapshot(db));

    async function request(method: string, path: string) {
      const response = await mf.dispatchFetch(`http://microblog.test/${path}`, {
        method,
      });
      return { status: response.status, body: await response.json() };
    }

    assert.equal((await request("POST", "follow/Reader/Author")).status, 200);
    assert.equal((await request("POST", "posts/Earlier/Author")).status, 200);
    assert.equal((await request("POST", "relay/Earlier")).status, 200);
    await acked;

    const deliveries = events.filter(
      ({ command }) => command.kind === "queue_deliver",
    );
    const ids = deliveries.map(({ message }) => message?.id);
    assert.equal(ids.length, 2);
    assert.ok(ids[0]);
    assert.equal(ids[1], ids[0]);
    assert.deepEqual(
      deliveries.map(({ message }) => message?.attempts),
      [1, 2],
    );
    assert.deepEqual(
      events
        .filter(({ command }) => command.kind === "queue_ack")
        .map(({ command }) => command),
      [{ kind: "queue_ack", p: "Earlier", delivery: "Retry" }],
    );
    assert.deepEqual(await request("GET", "timeline/Reader/Earlier"), {
      status: 200,
      body: { p: "Earlier", author: "Author" },
    });

    assert.equal((await request("DELETE", "follow/Reader/Author")).status, 200);
    assert.equal((await request("GET", "timeline/Reader/Earlier")).status, 404);
    assert.deepEqual((await snapshot(db)).entries.Reader, ["Earlier"]);

    const recording = fromReplayItf(
      JSON.parse(
        JSON.stringify(
          toReplayItf({
            scenario: {
              model,
              config,
              initial,
              commands: events.map(({ command }) => command),
            },
            observations,
          }),
        ),
      ),
    );
    const result = await replayRecording(
      checker,
      recording,
      ({ authors, follows, entries, applied }) => ({
        authors,
        follows,
        entries,
        applied,
      }),
    );
    assert.deepEqual(result, { verdict: "matched" });
  },
);
