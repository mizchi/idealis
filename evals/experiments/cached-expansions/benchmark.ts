import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  access,
  cp,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { names } from "../quint-generated/cases.js";
import { createRunDirectory } from "./directory.js";

const [output, baseline] = process.argv.slice(2);
assert.ok(output && baseline && /^[a-z0-9-]+$/.test(baseline));
await createRunDirectory(output);
const expectedNames = [
  ...names,
  "derived-fold-16",
  "derived-fold-256",
  "derived-fold-4096",
];
const stages = ["uncached", "cold", "warm", "snapshot"];
const digest = (body: string | Uint8Array) =>
  createHash("sha256").update(body).digest("hex");
const sourcePaths = [
  "crates/rtlc/src/expansions.rs",
  "crates/rtlc/src/safety.rs",
  "crates/rtlc/src/safety_values.rs",
  "crates/rtlc/src/safety_parallel.rs",
  "crates/rtlc/Cargo.toml",
  "crates/rtlc/Cargo.lock",
  "crates/rtlc/src/expansion_benchmarks.rs",
  "crates/rtlc/tests/expansions-fixtures.json",
  "evals/experiments/cached-expansions/benchmark.ts",
  "evals/experiments/cached-expansions/directory.ts",
];
const sources = [];
for (const path of sourcePaths) {
  const body = await readFile(path);
  await mkdir(resolve(output, "sources", dirname(path)), { recursive: true });
  await writeFile(resolve(output, "sources", path), body);
  sources.push({ path, sha256: digest(body) });
}
const inputs = JSON.parse(
  await readFile("crates/rtlc/tests/expansions-fixtures.json", "utf8"),
);
assert.deepEqual(
  inputs.map((input: { name: string }) => input.name),
  expectedNames,
);
for (const stage of stages)
  for (const name of expectedNames) {
    const path = `crates/rtlc/target/criterion/expansions/${stage}/${name}/${baseline}`;
    await assert.rejects(access(path), { code: "ENOENT" });
  }
const args = [
  "bench",
  "--locked",
  "--manifest-path",
  "crates/rtlc/Cargo.toml",
  "--bench",
  "expansions",
  "--",
  "--sample-size",
  "10",
  "--warm-up-time",
  "0.1",
  "--measurement-time",
  "0.3",
  "--noplot",
  "--save-baseline",
  baseline,
];
const manifest = {
  plan: "evals/plans/cached-expansions.md",
  sources,
  names: expectedNames,
  stages,
  args,
  baseline,
  rustc: spawnSync("rustc", ["--version"], { encoding: "utf8" }).stdout.trim(),
  platform: process.platform,
  arch: process.arch,
  sampleSize: 10,
  timingScope:
    "native prepared program; warm clone setup excluded; snapshot stage includes JSON deserialize and serialize; no source compile or process startup",
};
await writeFile(
  resolve(output, "manifest.json"),
  JSON.stringify(manifest, null, 2),
);
const run = spawnSync("cargo", args, {
  encoding: "utf8",
  maxBuffer: 16 * 1024 * 1024,
});
await writeFile(resolve(output, "stdout.log"), run.stdout ?? "");
await writeFile(resolve(output, "stderr.log"), run.stderr ?? "");
if (run.error) throw run.error;
assert.equal(run.status, 0, run.stderr);
const rows = [];
const files = [];
for (const stage of stages)
  for (const name of expectedNames) {
    const source = `crates/rtlc/target/criterion/expansions/${stage}/${name}/${baseline}`;
    const destination = resolve(output, "raw", stage, name);
    await cp(source, destination, { recursive: true, errorOnExist: true });
    for (const file of await readdir(destination))
      files.push({
        path: `raw/${stage}/${name}/${file}`,
        sha256: digest(await readFile(resolve(destination, file))),
      });
    const estimates = JSON.parse(
      await readFile(resolve(destination, "estimates.json"), "utf8"),
    );
    const sample = JSON.parse(
      await readFile(resolve(destination, "sample.json"), "utf8"),
    );
    assert.equal(sample.iters.length, 10);
    assert.equal(sample.times.length, 10);
    assert.ok(estimates.median.point_estimate > 0);
    rows.push({
      name,
      stage,
      medianUs: estimates.median.point_estimate / 1000,
      lowerUs: estimates.median.confidence_interval.lower_bound / 1000,
      upperUs: estimates.median.confidence_interval.upper_bound / 1000,
    });
  }
for (const input of sources)
  assert.equal(digest(await readFile(input.path)), input.sha256, input.path);
await writeFile(
  resolve(output, "result.json"),
  JSON.stringify({ status: "complete", rows, files }, null, 2),
);
console.log(JSON.stringify(rows, null, 2));
