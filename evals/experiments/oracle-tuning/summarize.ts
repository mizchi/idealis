import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

interface Measurement {
  readonly id: string;
  readonly statistics: { readonly median_ns: number };
}
interface Roundtrip {
  readonly status: string;
  readonly measurement: boolean;
  readonly records: readonly {
    readonly stage: string;
    readonly cases: readonly {
      readonly id: string;
      readonly measurements: readonly Measurement[];
    }[];
  }[];
}
interface Estimate {
  readonly median: { readonly point_estimate: number };
}
const roundtrip = process.argv[2];
const kernel = process.argv[3];
const output = process.argv[4];
assert.ok(roundtrip && kernel && output);
const result: Roundtrip = JSON.parse(
  await readFile(join(roundtrip, "result.json"), "utf8"),
);
assert.equal(result.status, "complete");
assert.equal(result.measurement, true);
const median = (values: readonly number[]) => {
  assert.ok(values.length > 0);
  const sorted = [...values].sort((left, right) => left - right);
  return (
    ((sorted[Math.floor((sorted.length - 1) / 2)] ?? NaN) +
      (sorted[Math.floor(sorted.length / 2)] ?? NaN)) /
    2
  );
};
const direct = result.records.some((record) => record.stage === "wire");
const names = direct
  ? ["before", "wire", "combined"]
  : ["before", "transport", "bridge"];
const roundtrips = ["rtla", "idealis-core"].map((id) => {
  const stages = names.map((stage) => {
    const records = result.records.filter((record) => record.stage === stage);
    assert.equal(records.length, 2);
    const measurements = ["baseline", "candidate"].map((implementation) => {
      const runs = records.map((record) => {
        const value = record.cases
          .find((item) => item.id === id)
          ?.measurements.find((item) => item.id === implementation)
          ?.statistics.median_ns;
        assert.ok(value !== undefined && value > 0);
        return value;
      });
      return { implementation, runsMedianNs: runs, medianNs: median(runs) };
    });
    return { stage, measurements };
  });
  const candidates = stages.map(
    (stage) => stage.measurements[1]?.medianNs ?? NaN,
  );
  return {
    id,
    stages,
    stageSpeedups: [
      {
        from: names[0],
        to: names[1],
        speedup: (candidates[0] ?? NaN) / (candidates[1] ?? NaN),
      },
      {
        from: names[1],
        to: names[2],
        speedup: (candidates[1] ?? NaN) / (candidates[2] ?? NaN),
      },
    ],
    totalSpeedup: (candidates[0] ?? NaN) / (candidates[2] ?? NaN),
  };
});
const kernels = await Promise.all(
  ["wire-value", "wire-bytes"].flatMap((mode) =>
    ["single", "batch-32", "nested-256", "unknown"].map(async (name) => {
      const runs = await Promise.all(
        [0, 1, 2, 3].map(async (index) => {
          const estimate: Estimate = JSON.parse(
            await readFile(
              join(
                kernel,
                String(index),
                "criterion",
                `${mode}_${name}`,
                "new/estimates.json",
              ),
              "utf8",
            ),
          );
          return estimate.median.point_estimate;
        }),
      );
      const beforeNs = median([runs[0] ?? NaN, runs[3] ?? NaN]);
      const afterNs = median([runs[1] ?? NaN, runs[2] ?? NaN]);
      return {
        mode,
        name,
        runsMedianNs: runs,
        beforeNs,
        afterNs,
        speedup: beforeNs / afterNs,
      };
    }),
  ),
);
const manifest = JSON.parse(
  await readFile(join(roundtrip, "manifest.json"), "utf8"),
);
const snapshots = await Promise.all(
  manifest.snapshots.map(
    async (item: { readonly path: string; readonly sha256: string }) => {
      const bytes = await readFile(join(item.path, "manifest.json"));
      assert.equal(
        createHash("sha256").update(bytes).digest("hex"),
        item.sha256,
      );
      return JSON.parse(bytes.toString());
    },
  ),
);
await mkdir(output);
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify({ status: "complete", method: "median of two run medians, descriptive ratios without significance claim", roundtrip, kernel, roundtrips, kernels, snapshots }, null, 2)}\n`,
  { flag: "wx" },
);
