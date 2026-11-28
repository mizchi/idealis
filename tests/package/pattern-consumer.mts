import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import {
  browserHistoryPattern,
  fromReplayItf,
  nodeFileIoBatch,
  nodeFileNextTickBatch,
  nodeIoBatchPattern,
  nodeNextTickPattern,
  objectStorePattern,
  objectStoreProjection,
  recordReplay,
  repeatedTemporalScenario,
  replayRecording,
  s3ObjectStore,
  toReplayItf,
  type BrowserHistory,
  type S3ObjectClient,
} from "@mizchi/idealis/patterns";

const rcheck = process.argv[2];
assert.ok(rcheck);

const file = new URL(import.meta.url);
const io = nodeIoBatchPattern(nodeFileIoBatch(file));
const ticks = nodeNextTickPattern(nodeFileNextTickBatch(file));
for (const pattern of [io, ticks]) {
  const result = await recordReplay(rcheck, pattern.scenario, pattern.adapter);
  assert.equal(result.verdict, "matched");
}

const routes = {
  Home: { path: "/home", screen: "home" },
  Catalog: { path: "/catalog", screen: "catalog" },
  Detail: { path: "/items/42", screen: "item" },
};
let cursor = 0;
const entries = [routes.Home.path];
const history: BrowserHistory = {
  async push(path) {
    entries.splice(cursor + 1, entries.length, path);
    cursor += 1;
  },
  async goBack() {
    cursor -= 1;
  },
  async goForward() {
    cursor += 1;
  },
  async observe() {
    const path = entries[cursor];
    const screen = Object.values(routes).find((route) => route.path === path);
    assert.ok(screen);
    return screen;
  },
};
const browsing = browserHistoryPattern(history, routes);
assert.equal(
  (await recordReplay(rcheck, browsing.scenario, browsing.adapter)).verdict,
  "matched",
);

let object: string | null = null;
const client: S3ObjectClient = {
  async putObject({ Body }) {
    // A controlled stale read after an overwrite.
    if (object === null) object = Body;
  },
  async deleteObject() {
    object = null;
  },
  async getObject() {
    if (object === null) {
      const error = new Error("absent");
      error.name = "NoSuchKey";
      throw error;
    }
    const body = object;
    return {
      Body: {
        async transformToString() {
          return body;
        },
      },
    };
  },
  async listObjectsV2({ Prefix }) {
    return { Contents: object === null ? [] : [{ Key: Prefix }] };
  },
};
const store = objectStorePattern(s3ObjectStore(client, "test"), "probe");
const recorded = await recordReplay(rcheck, store.scenario, store.adapter);
assert.equal(recorded.verdict, "mismatch");
assert.equal(recorded.difference.step, 2);
assert.deepEqual(recorded.difference.expected, { body: "V2", listed: true });
assert.deepEqual(recorded.difference.observed, { body: "V1", listed: true });
await writeFile(
  "recording.itf.json",
  JSON.stringify(toReplayItf(recorded.recording)),
);
const saved = fromReplayItf(
  JSON.parse(await readFile("recording.itf.json", "utf8")),
);
assert.deepEqual(await replayRecording(rcheck, saved, objectStoreProjection), {
  verdict: "mismatch",
  difference: recorded.difference,
});
const fifo = await readFile("weighted-fifo.model", "utf8");
const bypass = fifo.replace(
  "client = at(waiting, 0),",
  "client in elements(waiting),",
);
assert.notEqual(bypass, fifo);
await writeFile("weighted-bypass.model", bypass);
const finite = await repeatedTemporalScenario<{ kind: string; client: string }>(
  rcheck,
  "weighted-bypass.model",
  "HeadGetsCapacity",
  2,
);
assert.ok(
  finite.commands.filter(
    ({ kind, client }) => kind === "grant" && client === "Light",
  ).length >= 2,
);
assert.ok(
  !finite.commands.some(
    ({ kind, client }) => kind === "grant" && client === "Heavy",
  ),
);
console.log(
  "packaged patterns replayed; persisted overwrite mismatch and weighted wait loop reproduced",
);
