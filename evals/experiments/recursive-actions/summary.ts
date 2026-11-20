import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { unzipSync } from "fflate";
import { digest } from "../usable/provenance.js";

interface Estimate {
  readonly name: string;
  readonly estimates: { readonly mean: { readonly point_estimate: number } };
}
interface Measurement {
  readonly status: string;
  readonly estimates: readonly Estimate[];
}
interface Snapshot {
  readonly sourceZipSha256: string;
  readonly toolchain: string;
  readonly environment: unknown;
}
interface Run {
  readonly snapshot: string;
  readonly buildManifestSha256: string;
}
const [directory, before, after, ...runs] = process.argv.slice(2);
assert.ok(directory && before && after && runs.length === 8);
await mkdir(directory);
const save = (name: string, value: unknown) =>
  writeFile(`${directory}/${name}`, `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const bytes = await Promise.all(
  [before, after].map((root) => readFile(`${root}/source.zip`)),
);
const archives = bytes.map((bytes) => unzipSync(bytes));
const left = archives[0];
const right = archives[1];
assert.ok(left && right);
const manifests = await Promise.all(
  [before, after].map((root) => readFile(`${root}/manifest.json`)),
);
const snapshots: Snapshot[] = manifests.map((bytes) =>
  JSON.parse(bytes.toString()),
);
assert.deepEqual(snapshots[0]?.toolchain, snapshots[1]?.toolchain);
assert.deepEqual(snapshots[0]?.environment, snapshots[1]?.environment);
bytes.forEach((value, index) => {
  assert.equal(digest(value), snapshots[index]?.sourceZipSha256);
});
assert.deepEqual(Object.keys(left).sort(), Object.keys(right).sort());
const changes = Object.keys(left).filter((path) => {
  const before = left[path];
  const after = right[path];
  assert.ok(before && after);
  return digest(before) !== digest(after);
});
assert.deepEqual(changes, ["crates/rtlc/src/safety_expressions.rs"]);
const input = await Promise.all(
  runs.map(async (path, index) => {
    const manifest: Run = JSON.parse(
      await readFile(`${path}/manifest.json`, "utf8"),
    );
    const snapshot = index < 4 ? before : after;
    assert.equal(resolve(manifest.snapshot), resolve(snapshot));
    const expected = manifests[index < 4 ? 0 : 1];
    assert.ok(expected);
    assert.equal(manifest.buildManifestSha256, digest(expected));
    const result = await readFile(`${path}/result.json`);
    const value: Measurement = JSON.parse(result.toString());
    assert.equal(value.status, "complete");
    assert.equal(value.estimates.length, 21);
    return { path, resultSha256: digest(result), value };
  }),
);
const median = (values: readonly number[]) => {
  const sorted = [...values].sort((left, right) => left - right);
  assert.equal(sorted.length, 4);
  const lower = sorted[1];
  const upper = sorted[2];
  assert.ok(lower !== undefined && upper !== undefined);
  return (lower + upper) / 2;
};
const first = input[0];
assert.ok(first);
const records = first.value.estimates.map((estimate) => {
  const times = input.map((record) => {
    const found = record.value.estimates.find(
      (row) => row.name === estimate.name,
    );
    assert.ok(found);
    return found.estimates.mean.point_estimate / 1000;
  });
  const baseline = times.filter((_, index) => index < 4);
  const candidate = times.filter((_, index) => index >= 4);
  const beforeUs = median(baseline);
  const afterUs = median(candidate);
  return {
    name: estimate.name.replace("/new/estimates.json", ""),
    baseline,
    candidate,
    beforeUs,
    afterUs,
    changePercent: (afterUs / beforeUs - 1) * 100,
    speedup: beforeUs / afterUs,
  };
});
await save("manifest.json", {
  plan: "0071-recursive-actions",
  before,
  after,
  sourceZipSha256: bytes.map(digest),
  changes,
  driverSha256: digest(
    await readFile("evals/experiments/recursive-actions/summary.ts"),
  ),
  runs: input.map(({ path, resultSha256 }) => ({ path, resultSha256 })),
  order:
    "ABBA then BAAB. Arguments list baseline runs first, then candidate runs.",
  aggregation:
    "Median of four Criterion per-run mean estimates. Keep every repetition, including outliers. Nonexclusive host.",
});
await save("result.json", {
  status: "complete",
  records,
  scope:
    "Kernel-only comparison of immutable binaries. Same seven inputs, toolchain and benchmark. Runtime parent sharing is the only source difference. Compile timings are controls, not a compilation speedup claim.",
});
