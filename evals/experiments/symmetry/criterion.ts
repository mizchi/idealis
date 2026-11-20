import assert from "node:assert/strict";
import { type SpawnSyncReturns, spawnSync } from "node:child_process";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { digest } from "../usable/provenance.js";
import { symmetryFingerprint } from "./provenance.js";

const [binary, directory, filter] = process.argv.slice(2);
assert.ok(binary && directory);
await mkdir(directory);
const source = await symmetryFingerprint();
const binarySha256 = digest(await readFile(binary));
const files: string[] = [];
const save = async (path: string, value: unknown) => {
  await writeFile(
    join(directory, path),
    `${JSON.stringify(value, null, 2)}\n`,
    { flag: "wx" },
  );
  files.push(path);
};
await save("manifest.json", {
  plan: process.env.IDEALIS_SYMMETRY_PLAN ?? "0061-symmetry-next",
  ...source,
  binary,
  binarySha256,
  sampleSize: 20,
  warmupMs: 500,
  measurementMs: 1000,
  reverse: [false, true],
  benchSourceSha256: digest(
    await readFile("crates/rtlc/src/symmetry_benchmarks.rs"),
  ),
  fixtureSha256: digest(
    await readFile("crates/rtlc/src/symmetry-fixtures.json"),
  ),
  reductionFixtureSha256: digest(
    await readFile("crates/rtlc/src/reduction-fixtures.json"),
  ),
});
for (const index of [0, 1]) {
  const output = resolve(directory, String(index));
  await mkdir(output);
  const result: SpawnSyncReturns<string> = spawnSync(
    resolve(binary),
    ["--bench", ...(filter ? [filter] : [])],
    {
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
      env: {
        ...process.env,
        CARGO_TARGET_DIR: output,
        ...(index === 1 ? { IDEALIS_SYMMETRY_REVERSE: "1" } : {}),
      },
    },
  );
  await writeFile(join(output, "process.log"), result.stdout + result.stderr, {
    flag: "wx",
  });
  assert.equal(result.status, 0, result.stderr);
}
const find = async (directory: string): Promise<string[]> =>
  (
    await Promise.all(
      (await readdir(directory, { withFileTypes: true })).map(
        async (entry): Promise<string[]> => {
          const path = join(directory, entry.name);
          return entry.isDirectory()
            ? find(path)
            : entry.name === "estimates.json" && directory.endsWith("/new")
              ? [path]
              : [];
        },
      ),
    )
  ).flat();
const estimates = await find(join(directory, "0/criterion"));
const fixtures: { name: string }[] = JSON.parse(
  await readFile("crates/rtlc/src/symmetry-fixtures.json", "utf8"),
);
fixtures.push(
  ...JSON.parse(
    await readFile("crates/rtlc/src/reduction-fixtures.json", "utf8"),
  ),
);
assert.equal(
  estimates.length,
  fixtures.filter(
    (fixture) => !filter || new RegExp(filter).test(`symmetry/${fixture.name}`),
  ).length,
);
const kernels = await Promise.all(
  estimates.sort().map(async (path) => {
    const name = relative(join(directory, "0/criterion"), path);
    const means = await Promise.all(
      [0, 1].map(async (index) => {
        const value: { mean: { point_estimate: number } } = JSON.parse(
          await readFile(
            join(directory, String(index), "criterion", name),
            "utf8",
          ),
        );
        assert.ok(value.mean.point_estimate > 0);
        return value.mean.point_estimate;
      }),
    );
    return {
      name,
      meanNanoseconds: means,
      meanMs: ((means[0] ?? NaN) + (means[1] ?? NaN)) / 2e6,
    };
  }),
);
assert.equal((await symmetryFingerprint()).sourceVersion, source.sourceVersion);
assert.equal(digest(await readFile(binary)), binarySha256);
await save("result.json", {
  status: "complete",
  kernels,
  method:
    "arithmetic mean of two Criterion mean estimates; no pooled confidence interval",
  files: await Promise.all(
    files.map(async (path) => ({
      path,
      sha256: digest(await readFile(join(directory, path))),
    })),
  ),
});
