import assert from "node:assert/strict";
import {
  appendFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { basename, join, relative } from "node:path";
import { digest } from "../usable/provenance.js";

const [input, output, expected, plan = "0063-evaluator-tuning"] =
  process.argv.slice(2);
assert.ok(input && output && expected);
const count = Number(expected);
assert.ok(Number.isSafeInteger(count) && count > 0);
const find = async (directory: string): Promise<string[]> =>
  (
    await Promise.all(
      (
        await readdir(directory, { withFileTypes: true })
      ).map(async (entry) => {
        const path = join(directory, entry.name);
        if (entry.isDirectory()) return find(path);
        return entry.name === "estimates.json" && directory.endsWith("/new")
          ? [path]
          : [];
      }),
    )
  ).flat();
const paths = (await find(join(input, "0/criterion"))).sort();
assert.equal(paths.length, count);
const records = await Promise.all(
  paths.map(async (path) => {
    const name = relative(join(input, "0/criterion"), path);
    const estimates = await Promise.all(
      [0, 1, 2, 3].map(async (index) => {
        const path = join(input, String(index), "criterion", name);
        const raw = await readFile(path);
        const data: {
          mean: {
            point_estimate: number;
            confidence_interval: { lower_bound: number; upper_bound: number };
          };
        } = JSON.parse(raw.toString());
        assert.ok(Number.isFinite(data.mean.point_estimate));
        assert.ok(data.mean.point_estimate > 0);
        return { path, sha256: digest(raw), mean: data.mean };
      }),
    );
    const means = estimates.map((estimate) => estimate.mean.point_estimate);
    const beforeNs = ((means[0] ?? NaN) + (means[3] ?? NaN)) / 2;
    const afterNs = ((means[1] ?? NaN) + (means[2] ?? NaN)) / 2;
    return {
      name: name.replace("/new/estimates.json", ""),
      estimates,
      beforeNs,
      afterNs,
      ratio: afterNs / beforeNs,
      regressionOver10Percent: afterNs > beforeNs * 1.1,
    };
  }),
);
const manifest = await readFile(join(input, "manifest.json"));
await mkdir(output);
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify({ status: "complete", plan, input, manifest: JSON.parse(manifest.toString()), manifestSha256: digest(manifest), method: "Arithmetic mean of two Criterion mean estimates per side in ABBA order. Intervals are per repetition, not pooled.", records }, null, 2)}\n`,
  { flag: "wx" },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(output), plan, kind: "criterion-summary", status: "complete" })}\n`,
);
console.log(
  records.map((record) => ({ name: record.name, ratio: record.ratio })),
);
