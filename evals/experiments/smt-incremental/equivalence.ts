import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { digest } from "./provenance.js";
import { compactRecords, expandRecords } from "./records.js";

// equivalence <full-snapshot-run> <compact-run>
// Checks of evals/plans/0074c-smt-compact-cache-records.md: same binaries,
// same summary, and the full records compact to the new ones.
const [full, compact] = process.argv.slice(2);
assert.ok(full && compact);
const load = async (directory: string) => {
  const json = async (path: string) =>
    JSON.parse(await readFile(join(directory, path), "utf8"));
  const manifest = await json("manifest.json");
  const result = await json("result.json");
  assert.equal(result.status, "complete", directory);
  for (const file of result.files)
    assert.equal(
      digest(await readFile(join(directory, file.path))),
      file.sha256,
      file.path,
    );
  const text = await readFile(join(directory, "records.jsonl"), "utf8");
  const records = text
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line) as Record<string, unknown>);
  return { manifest, result, records, bytes: Buffer.byteLength(text) };
};
const before = await load(full);
const after = await load(compact);
assert.equal(after.manifest.recordFormat, "0074c-smt-compact-cache-records");
assert.equal(before.manifest.phase, after.manifest.phase);
assert.equal(before.manifest.seed, after.manifest.seed);
assert.deepEqual(after.manifest.binaries, before.manifest.binaries);
assert.deepEqual(after.result.summary, before.result.summary);
// Z3 may choose another valid model, as in replay.
const deterministic = (entries: readonly Record<string, unknown>[]) =>
  entries.map((entry) =>
    entry.reference
      ? {
          ...entry,
          reference: { status: (entry.reference as { status: string }).status },
        }
      : entry,
  );
assert.deepEqual(
  deterministic(compactRecords(before.records) as Record<string, unknown>[]),
  deterministic(after.records),
);
assert.deepEqual(expandRecords(after.records).length, before.records.length);
assert.deepEqual(
  deterministic(expandRecords(after.records) as Record<string, unknown>[]),
  deterministic(before.records),
);
console.log(
  JSON.stringify({
    status: "equivalent",
    phase: after.manifest.phase,
    fullBytes: before.bytes,
    compactBytes: after.bytes,
    ratio: after.bytes / before.bytes,
  }),
);
