import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { digest } from "../usable/provenance.js";

const before = process.argv[2];
const after = process.argv[3];
const directory = process.argv[4];
assert.ok(before && after && directory);
await mkdir(directory);
const read = async (path: string) => JSON.parse(await readFile(path, "utf8"));
interface Run {
  readonly status: string;
  readonly sourceUnchanged: boolean;
  readonly files: readonly { readonly path: string; readonly sha256: string }[];
  readonly estimates: readonly Estimate[];
}
const baseline: Run = await read(join(before, "result.json"));
const candidate: Run = await read(join(after, "result.json"));
assert.equal(baseline.status, "complete");
assert.equal(candidate.status, "complete");
assert.equal(baseline.sourceUnchanged, true);
assert.equal(candidate.sourceUnchanged, true);
for (const run of [before, after]) {
  const result: Run = run === before ? baseline : candidate;
  for (const file of result.files)
    assert.equal(
      digest(await readFile(join(run, file.path))),
      file.sha256,
      file.path,
    );
}
const left = await read(join(before, "manifest.json"));
const right = await read(join(after, "manifest.json"));
for (const field of [
  "target",
  "benchmarkSuite",
  "sampleSize",
  "warmupSeconds",
  "measurementSeconds",
])
  assert.deepEqual(left[field], right[field]);
for (const path of [
  "inputs.json",
  ...(left.target === "rtla-compiler"
    ? ["module-inputs.json", "operator-inputs.json"]
    : []),
])
  assert.deepEqual(
    await read(join(before, path)),
    await read(join(after, path)),
  );
interface Estimate {
  readonly benchmark: string;
  readonly mean: number;
  readonly lower: number;
  readonly upper: number;
}
const previous = new Map<string, Estimate>(
  baseline.estimates.map((row: Estimate) => [row.benchmark, row]),
);
const current = candidate.estimates;
assert.equal(current.length, previous.size);
const comparisons = current.map((row) => {
  const old = previous.get(row.benchmark);
  assert.ok(old);
  return {
    benchmark: row.benchmark,
    before: old,
    after: row,
    speedup: old.mean / row.mean,
    regression: row.mean > old.mean * 1.03,
  };
});
await writeFile(
  join(directory, "result.json"),
  JSON.stringify(
    {
      status: "complete",
      before,
      after,
      inputsEqual: true,
      sourceResults: await Promise.all(
        [before, after].map(async (path) => ({
          path: join(path, "result.json"),
          sha256: digest(await readFile(join(path, "result.json"))),
        })),
      ),
      comparisons,
      regressions: comparisons
        .filter((row) => row.regression)
        .map((row) => row.benchmark),
      interpretation:
        "Ratios of Criterion mean estimates. A regression flag is a >3% slowdown, not a statistical significance test or an adoption decision.",
    },
    null,
    2,
  ),
  { flag: "wx" },
);
console.log(
  JSON.stringify(
    comparisons.map((row) => ({
      benchmark: row.benchmark,
      speedup: row.speedup,
      regression: row.regression,
    })),
    null,
    2,
  ),
);
