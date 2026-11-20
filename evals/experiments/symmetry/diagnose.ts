import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus } from "node:os";
import { join, resolve } from "node:path";

const [before, after, directory, model = "linear-ring"] = process.argv.slice(2);
assert.ok(before && after && directory);
await mkdir(directory);
const binaries = { before: resolve(before), after: resolve(after) };
const hashes = await Promise.all(
  [before, after].map(async (path) =>
    createHash("sha256")
      .update(await readFile(path))
      .digest("hex"),
  ),
);
const order = [
  "before",
  "after",
  "after",
  "before",
  "after",
  "before",
  "before",
  "after",
] as const;
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "0061-symmetry-next",
      binaries,
      hashes,
      order,
      cpu: cpus()[0]?.model,
      filter: `search/${model}`,
      sampleSize: 20,
      warmupSeconds: 0.5,
      measurementSeconds: 1,
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const measurements = [];
for (const [index, name] of order.entries()) {
  const output = resolve(directory, String(index));
  await mkdir(output);
  const result = spawnSync(
    binaries[name],
    [
      "--bench",
      `search/${model}`,
      "--sample-size",
      "20",
      "--warm-up-time",
      "0.5",
      "--measurement-time",
      "1",
    ],
    {
      encoding: "utf8",
      env: { ...process.env, CARGO_TARGET_DIR: output },
    },
  );
  await writeFile(join(output, "process.log"), result.stdout + result.stderr, {
    flag: "wx",
  });
  assert.equal(result.status, 0, result.stderr);
  for (const workers of [1, 8]) {
    const raw = await readFile(
      join(output, `criterion/search/${model}/${workers}/new/estimates.json`),
      "utf8",
    );
    const estimate: { mean: { point_estimate: number } } = JSON.parse(raw);
    measurements.push({
      index,
      name,
      workers,
      meanMs: estimate.mean.point_estimate / 1e6,
    });
  }
}
await writeFile(
  join(directory, "result.json"),
  JSON.stringify({ status: "complete", measurements }, null, 2),
  { flag: "wx" },
);
console.log(JSON.stringify(measurements));
