import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const directory = process.argv[2];
assert.ok(directory);
const evidence = JSON.parse(
  await readFile(resolve(directory, "evidence.json"), "utf8"),
);
for (const entry of evidence) {
  assert.equal(
    createHash("sha256")
      .update(await readFile(resolve(directory, entry.path)))
      .digest("hex"),
    entry.sha256,
    `${entry.path}: saved evidence changed`,
  );
}
const manifest = JSON.parse(
  await readFile(resolve(directory, "manifest.json"), "utf8"),
);
for (const entry of manifest.fingerprints) {
  assert.equal(
    createHash("sha256")
      .update(await readFile(entry.path))
      .digest("hex"),
    entry.sha256,
    `${entry.path}: provenance changed`,
  );
}
for (const name of ["cold-bench", "prepared-bench"]) {
  const report = JSON.parse(
    await readFile(resolve(directory, `${name}.json`), "utf8"),
  );
  assert.equal(report.status, "matched");
  assert.equal(report.evaluations, report.planned_evaluations);
  for (const sample of report.cases) {
    assert.equal(sample.order.length, report.config.samples);
    const baseline = sample.measurements.find(
      (entry: { id: string }) => entry.id === report.config.baseline,
    );
    assert.ok(baseline);
    for (const measurement of sample.measurements) {
      assert.equal(measurement.samples_ns.length, report.config.samples);
      assert.ok(measurement.samples_ns.every((value: number) => value > 0));
      const sorted = measurement.samples_ns
        .map((value: number) => value / report.config.iterations)
        .sort((left: number, right: number) => left - right);
      const middle = Math.floor(sorted.length / 2);
      const median =
        sorted.length % 2 === 0
          ? (sorted[middle - 1] + sorted[middle]) / 2
          : sorted[middle];
      assert.equal(measurement.statistics.median_ns, median);
      assert.equal(measurement.speedup, baseline.statistics.median_ns / median);
    }
  }
}
execFileSync(
  resolve(
    "evals/experiments/oracle-comparison/rust/target/release/oracle-comparison",
  ),
  [resolve(directory), "--replay"],
  { stdio: "inherit" },
);
