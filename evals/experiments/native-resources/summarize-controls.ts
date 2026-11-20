import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { join, relative } from "node:path";

const directory = process.argv[2];
assert.ok(directory);
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
const paths = (await find(join(directory, "0/criterion"))).sort();
assert.equal(paths.length, 14);
const kernels = [];
const files: { path: string; sha256: string }[] = [];
for (const path of paths) {
  const name = relative(join(directory, "0/criterion"), path);
  const means = [];
  for (const index of [0, 1, 2, 3]) {
    const file = `${index}/criterion/${name}`;
    const bytes = await readFile(join(directory, file));
    const estimate: { mean: { point_estimate: number } } = JSON.parse(
      bytes.toString(),
    );
    means.push(estimate.mean.point_estimate / 1e6);
    files.push({
      path: file,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    });
  }
  const beforeMs = ((means[0] ?? NaN) + (means[3] ?? NaN)) / 2;
  const afterMs = ((means[1] ?? NaN) + (means[2] ?? NaN)) / 2;
  assert.ok(beforeMs > 0 && afterMs > 0);
  kernels.push({
    name,
    meansMs: means,
    beforeMs,
    afterMs,
    ratio: beforeMs / afterMs,
    timeIncreasePercent: (afterMs / beforeMs - 1) * 100,
  });
}
await writeFile(
  join(directory, "result.json"),
  `${JSON.stringify({ status: "complete", method: "ABBA arithmetic means, not pooled confidence intervals", kernels, files }, null, 2)}\n`,
  { flag: "wx" },
);
console.log(JSON.stringify(kernels));
