import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { names } from "../quint-generated/cases.js";

const directory = process.argv[2];
assert.ok(directory);
const result = JSON.parse(
  await readFile(resolve(directory, "result.json"), "utf8"),
);
const manifest = JSON.parse(
  await readFile(resolve(directory, "manifest.json"), "utf8"),
);
assert.equal(result.status, "complete");
assert.equal(result.rows.length, 36);
const expected = [
  ...names,
  "derived-fold-16",
  "derived-fold-256",
  "derived-fold-4096",
].flatMap((name) =>
  ["uncached", "cold", "warm", "snapshot"].map((stage) => `${stage}/${name}`),
);
assert.deepEqual(
  result.rows
    .map((row: { stage: string; name: string }) => `${row.stage}/${row.name}`)
    .sort(),
  expected.sort(),
);
for (const file of result.files)
  assert.equal(
    createHash("sha256")
      .update(await readFile(resolve(directory, file.path)))
      .digest("hex"),
    file.sha256,
    file.path,
  );
for (const file of manifest.sources)
  assert.equal(
    createHash("sha256")
      .update(await readFile(resolve(directory, "sources", file.path)))
      .digest("hex"),
    file.sha256,
    file.path,
  );
for (const row of result.rows) {
  const estimate = JSON.parse(
    await readFile(
      resolve(directory, "raw", row.stage, row.name, "estimates.json"),
      "utf8",
    ),
  );
  const sample = JSON.parse(
    await readFile(
      resolve(directory, "raw", row.stage, row.name, "sample.json"),
      "utf8",
    ),
  );
  assert.equal(sample.iters.length, 10);
  assert.equal(sample.times.length, 10);
  assert.equal(row.medianUs, estimate.median.point_estimate / 1000);
  assert.equal(
    row.lowerUs,
    estimate.median.confidence_interval.lower_bound / 1000,
  );
  assert.equal(
    row.upperUs,
    estimate.median.confidence_interval.upper_bound / 1000,
  );
}
console.log(
  "Replayed 36 Criterion measurements and their source/sample digests",
);
