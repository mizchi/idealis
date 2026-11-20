import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus } from "node:os";
import { join, resolve } from "node:path";

const before = process.argv[2];
const after = process.argv[3];
const output = process.argv[4];
const binaryName = process.argv[5] ?? "wire-bench";
const filter = process.argv[6];
assert.ok(
  binaryName === "wire-bench" ||
    binaryName === "safety-bench" ||
    binaryName === "symmetry-bench",
);
assert.ok(before && after && output);
const metadata = await Promise.all(
  [before, after].map(async (root) => {
    const manifest = JSON.parse(
      await readFile(join(root, "criterion.json"), "utf8"),
    );
    assert.equal(
      createHash("sha256")
        .update(await readFile(join(root, binaryName)))
        .digest("hex"),
      manifest.binarySha256,
    );
    return manifest;
  }),
);
assert.equal(metadata[0].benchSha256, metadata[1].benchSha256);
const inputPath =
  binaryName === "safety-bench"
    ? existsSync(".artifacts/criterion-safety/inputs.json")
      ? ".artifacts/criterion-safety/inputs.json"
      : "crates/rtlc/src/bench-fixtures.json"
    : undefined;
const input = inputPath ? await readFile(inputPath) : undefined;
const inputSha256 = input
  ? createHash("sha256").update(input).digest("hex")
  : undefined;
await mkdir(output);
if (input) await writeFile(join(output, "inputs.json"), input, { flag: "wx" });
await writeFile(
  join(output, "manifest.json"),
  `${JSON.stringify({ metadata, filter, inputPath, inputSha256, cpu: cpus()[0]?.model, sampleSize: 20, warmupMs: 500, measurementMs: 1000, order: ["before", "after", "after", "before"] }, null, 2)}\n`,
  { flag: "wx" },
);
for (const [index, root] of [before, after, after, before].entries()) {
  const directory = resolve(output, String(index));
  await mkdir(directory);
  const log = execFileSync(
    resolve(root, binaryName),
    ["--bench", ...(filter ? [filter] : [])],
    {
      env: { ...process.env, CARGO_TARGET_DIR: directory },
    },
  );
  await writeFile(join(directory, "stdout.log"), log, { flag: "wx" });
}
if (inputPath)
  assert.equal(
    createHash("sha256")
      .update(await readFile(inputPath))
      .digest("hex"),
    inputSha256,
    "Safety inputs changed during measurement",
  );
console.log(`Saved four isolated Criterion runs: ${output}`);
