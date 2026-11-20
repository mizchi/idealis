import assert from "node:assert/strict";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { cases } from "../single-model/cases.js";
import { digest } from "../usable/provenance.js";

interface Sample {
  readonly input: string;
  readonly route: string;
  readonly iteration: number;
  readonly warmup: boolean;
  readonly wallMs: number;
  readonly apiMs: number | null;
  readonly sampledMaxRssBytes: number | null;
}
interface Estimate {
  readonly mean: { readonly point_estimate: number };
}
interface Result {
  readonly status: string;
  readonly controls: readonly unknown[];
  readonly samples: readonly Sample[];
  readonly files: readonly { path: string; sha256: string }[];
}
const comparison = process.argv[2];
const criterion = process.argv[3];
const output = process.argv[4];
assert.ok(comparison && criterion && output);
const result: Result = JSON.parse(
  await readFile(join(comparison, "result.json"), "utf8"),
);
assert.equal(result.status, "complete");
assert.equal(result.controls.length, 72);
const manifest: { readonly models?: readonly string[] } = JSON.parse(
  await readFile(join(comparison, "manifest.json"), "utf8"),
);
const expectedModels = manifest.models ?? [
  "function-cube",
  "parallel-channels",
];
assert.equal(new Set(expectedModels).size, expectedModels.length);
assert.deepEqual(
  [...new Set(result.samples.map((sample) => sample.input))].sort(),
  [...expectedModels].sort(),
);
assert.equal(result.samples.length, expectedModels.length * 9 * 4);
assert.ok(result.files?.length);
for (const file of result.files)
  assert.equal(
    digest(await readFile(join(comparison, file.path))),
    file.sha256,
    file.path,
  );
const stats = (values: readonly number[]) => {
  assert.ok(
    values.length > 0 &&
      values.every((value) => Number.isFinite(value) && value > 0),
  );
  const sorted = [...values].sort((left, right) => left - right);
  return {
    median:
      ((sorted[Math.floor((sorted.length - 1) / 2)] ?? NaN) +
        (sorted[Math.floor(sorted.length / 2)] ?? NaN)) /
      2,
    min: sorted[0],
    max: sorted.at(-1),
    values,
  };
};
const models = cases("large")
  .filter((input) =>
    result.samples.some((sample) => sample.input === input.name),
  )
  .map((input) => {
    const selected = result.samples.filter(
      (sample) => sample.input === input.name && !sample.warmup,
    );
    const names = [...new Set(selected.map((sample) => sample.route))];
    assert.equal(names.length, 9);
    const routes = names.map((route) => {
      const samples = selected.filter((sample) => sample.route === route);
      assert.equal(samples.length, 3);
      assert.deepEqual(
        samples.map((sample) => sample.iteration).sort(),
        [1, 2, 3],
      );
      const wall = stats(samples.map((sample) => sample.wallMs));
      return {
        route,
        wallMs: wall,
        apiMs: samples.every((sample) => sample.apiMs !== null)
          ? stats(samples.map((sample) => sample.apiMs ?? NaN))
          : null,
        sampledMaxRssBytes: samples.every(
          (sample) => sample.sampledMaxRssBytes !== null,
        )
          ? stats(samples.map((sample) => sample.sampledMaxRssBytes ?? NaN))
          : null,
        generatedPerWallSecond: input.generated / (wall.median / 1000),
      };
    });
    const get = (route: string) => {
      const found = routes.find((entry) => entry.route === route);
      assert.ok(found);
      return found.wallMs.median;
    };
    return {
      name: input.name,
      states: input.states,
      generated: input.generated,
      routes,
      ratios: {
        native1BeforeAfter: get("before-native-1") / get("after-native-1"),
        native8BeforeAfter: get("before-native-8") / get("after-native-8"),
        wasmBeforeAfter: get("before-wasm") / get("after-wasm"),
        tlaVsNative1: get("tla-ts") / get("after-native-1"),
        tlc1VsNative1: get("tlc-1") / get("after-native-1"),
        tlc8VsNative8: get("tlc-8") / get("after-native-8"),
        tlaVsWasm: get("tla-ts") / get("after-wasm"),
      },
    };
  });
const findEstimates = async (directory: string): Promise<string[]> => {
  const results = await Promise.all(
    (await readdir(directory, { withFileTypes: true })).map(
      async (entry): Promise<string[]> => {
        const path = join(directory, entry.name);
        if (entry.isDirectory()) return findEstimates(path);
        return entry.name === "estimates.json" && directory.endsWith("/new")
          ? [path]
          : [];
      },
    ),
  );
  return results.flat();
};
const paths = await findEstimates(join(criterion, "0/criterion"));
assert.equal(paths.length, 14);
const kernels = await Promise.all(
  paths.sort().map(async (path) => {
    const name = relative(join(criterion, "0/criterion"), path);
    const means = await Promise.all(
      [0, 1, 2, 3].map(async (index) => {
        const estimate: Estimate = JSON.parse(
          await readFile(
            join(criterion, String(index), "criterion", name),
            "utf8",
          ),
        );
        assert.ok(estimate.mean.point_estimate > 0);
        return estimate.mean.point_estimate;
      }),
    );
    const before = ((means[0] ?? NaN) + (means[3] ?? NaN)) / 2;
    const after = ((means[1] ?? NaN) + (means[2] ?? NaN)) / 2;
    return {
      name: name.replace("/new/estimates.json", ""),
      means,
      beforeNs: before,
      afterNs: after,
      speedup: before / after,
      regressionOver10Percent: after > before * 1.1,
    };
  }),
);
await mkdir(output);
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify({ status: "complete", comparison, criterion, comparisonResultSha256: digest(await readFile(join(comparison, "result.json"))), criterionManifest: JSON.parse(await readFile(join(criterion, "manifest.json"), "utf8")), method: "wall: 3 sample median and full range; Criterion: arithmetic mean of two mean estimates, no pooled confidence interval", models, kernels }, null, 2)}\n`,
  { flag: "wx" },
);
console.log(
  models.map((model) => ({ name: model.name, ratios: model.ratios })),
);
