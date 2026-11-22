import assert from "node:assert/strict";
import { test } from "node:test";
import {
  compactRecords,
  expandRecords,
} from "../../evals/experiments/smt-incremental/records.js";

const header = {
  format: "rusmt.check-results",
  version: 1,
  buildDigest: "a".repeat(64),
};
const entry = (status: string) => ({ status, digest: status.repeat(4) });
const snapshot = (entries: Record<string, unknown>) => ({
  ...header,
  entries,
});
const reply = (entries: Record<string, unknown>) => ({
  kind: "incremental-solution",
  stats: { hits: 0, misses: 1, writes: 1, invalidated: false },
  cache: snapshot(entries),
});
const records = [
  { other: 1, replies: [{ cache: snapshot({ k: entry("x") }) }] },
  {
    contextCache: "a",
    replies: [
      { kind: "updated" },
      reply({ k1: entry("sat") }),
      reply({ k1: entry("sat"), k2: entry("unsat") }),
    ],
  },
  // The context stream continues across sequences.
  { contextCache: "b", replies: [reply({ k2: entry("unsat") })] },
  {
    scriptCache: "native",
    transcript: [reply({ s: entry("sat") }), reply({ s: entry("sat") })],
  },
  {
    scriptCache: "wasm",
    transcript: [reply({ s: entry("sat") }), reply({ t: entry("sat") })],
  },
];

test("cache records keep only the change from the previous snapshot of their stream", () => {
  const compact = compactRecords(records) as any[];
  assert.deepEqual(compact[0], records[0]);
  const [first, second] = compact[1].replies.slice(1).map((r: any) => r.cache);
  assert.deepEqual(first.header, header);
  assert.deepEqual(first.added, { k1: entry("sat") });
  assert.deepEqual(first.removed, []);
  assert.equal(first.entries, 1);
  assert.equal(second.header, undefined);
  assert.deepEqual(second.added, { k2: entry("unsat") });
  const third = compact[2].replies[0].cache;
  assert.deepEqual(third.added, {});
  assert.deepEqual(third.removed, ["k1"]);
  assert.equal(third.header, undefined);
  // Each engine starts its own stream.
  assert.deepEqual(compact[3].transcript[0].cache.header, header);
  assert.deepEqual(compact[3].transcript[1].cache.added, {});
  assert.deepEqual(compact[4].transcript[0].cache.header, header);
  assert.deepEqual(compact[4].transcript[1].cache.removed, ["s"]);
  assert.notEqual(
    compact[3].transcript[1].cache.digest,
    compact[4].transcript[1].cache.digest,
  );
});

test("expanding compact records restores every snapshot", () => {
  assert.deepEqual(expandRecords(compactRecords(records)), records);
});

test("a changed value and a changed header are recorded and restored", () => {
  const changed = [
    { contextCache: "a", replies: [reply({ k: entry("sat") })] },
    {
      contextCache: "b",
      replies: [
        {
          ...reply({ k: entry("unsat") }),
          cache: { ...header, version: 2, entries: { k: entry("unsat") } },
        },
      ],
    },
  ];
  const compact = compactRecords(changed) as any[];
  assert.deepEqual(compact[1].replies[0].cache.added, { k: entry("unsat") });
  assert.deepEqual(compact[1].replies[0].cache.header, {
    ...header,
    version: 2,
  });
  assert.deepEqual(expandRecords(compact), changed);
});

test("expansion rejects a record whose restored snapshot does not match its digest", () => {
  const compact = compactRecords(records) as any[];
  const tampered = structuredClone(compact);
  tampered[1].replies[2].cache.added = {};
  assert.throws(() => expandRecords(tampered), /digest/);
  const reordered = structuredClone(compact);
  [reordered[3], reordered[4]] = [reordered[4], reordered[3]];
  assert.doesNotThrow(() => expandRecords(reordered));
  const swapped = structuredClone(compact);
  [swapped[1], swapped[2]] = [swapped[2], swapped[1]];
  assert.throws(() => expandRecords(swapped));
});
