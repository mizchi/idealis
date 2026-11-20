import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { unzipSync } from "fflate";
import { digest } from "../usable/provenance.js";
import { benchmarkCases } from "./cases.js";

interface Manifest {
  readonly sourceZipSha256: string;
  readonly sources: readonly {
    readonly path: string;
    readonly sha256: string;
  }[];
}
const [snapshot, directory, output] = process.argv.slice(2);
assert.ok(snapshot && directory && output);
await mkdir(directory);
const manifest: Manifest = JSON.parse(
  await readFile(join(snapshot, "manifest.json"), "utf8"),
);
const archive = await readFile(join(snapshot, "source.zip"));
assert.equal(digest(archive), manifest.sourceZipSha256);
const files = unzipSync(archive);
for (const source of manifest.sources) {
  const value = files[source.path];
  assert.ok(value);
  assert.equal(digest(value), source.sha256, source.path);
}
const fixture = "crates/rtla-compiler/tests/frontend-semantics-bench.json";
const previous = files[fixture];
assert.ok(previous);
files[fixture] = new TextEncoder().encode(
  `${JSON.stringify(benchmarkCases, null, 2)}\n`,
);
await writeFile(
  join(directory, "manifest.json"),
  `${JSON.stringify({ plan: "0072-frontend-semantics", snapshot, sourceZipSha256: digest(archive), overlay: [{ path: fixture, before: digest(previous), after: digest(files[fixture]) }], reason: "Before the first timing sample, replace state-predicate temporal actuals unsupported by pinned TLC with constant actuals. Runtime/compiler sources stay at the frozen implementation before sharing." }, null, 2)}\n`,
  { flag: "wx" },
);
const root = resolve(directory, "source");
for (const [path, value] of Object.entries(files)) {
  assert.ok(path.startsWith("crates/") && !path.split("/").includes(".."));
  await mkdir(dirname(join(root, path)), { recursive: true });
  await writeFile(join(root, path), value, { flag: "wx" });
}
const driver = "evals/experiments/frontend-semantics/bench.ts";
await mkdir(dirname(join(root, driver)), { recursive: true });
await writeFile(join(root, driver), await readFile(driver), { flag: "wx" });
execFileSync("git", ["init", "-b", "bench-base"], { cwd: root, stdio: "pipe" });
execFileSync("git", ["add", "--", "."], { cwd: root });
const result = spawnSync(
  process.execPath,
  [
    resolve("dist/evals/experiments/frontend-semantics/bench.js"),
    "freeze",
    resolve(output),
  ],
  { cwd: root, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
);
await writeFile(join(directory, "build.log"), result.stdout + result.stderr, {
  flag: "wx",
});
assert.equal(result.status, 0, result.stderr);
