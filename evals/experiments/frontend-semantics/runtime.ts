import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { zipSync } from "fflate";
import { digest } from "../usable/provenance.js";

const directory = process.argv[2];
assert.ok(directory);
await mkdir(directory);
const paths = execFileSync(
  "git",
  [
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
    "--",
    "crates",
    "packages/idealis/src",
  ],
  { maxBuffer: 16 * 1024 * 1024 },
)
  .toString()
  .split("\0")
  .filter(
    (path) =>
      path &&
      !path.endsWith(".md") &&
      !path.endsWith("LICENSE") &&
      !path.endsWith("NOTICE"),
  )
  .sort();
const files = Object.fromEntries(
  await Promise.all(
    paths.map(async (path) => [path, await readFile(path)] as const),
  ),
);
const sources = Object.entries(files).map(([path, contents]) => ({
  path,
  sha256: digest(contents),
}));
const archive = zipSync(files, { level: 9 });
const save = (name: string, value: unknown) =>
  writeFile(join(directory, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
await writeFile(join(directory, "source.zip"), archive, { flag: "wx" });
await save("manifest.json", {
  plan: "0072-frontend-semantics",
  sources,
  sourceZipSha256: digest(archive),
  driverSha256: digest(
    await readFile("evals/experiments/frontend-semantics/runtime.ts"),
  ),
  toolchain: execFileSync("rustc", ["--version"]).toString().trim(),
  scope:
    "Freeze Rust build closure and TS contract sources before building native/Wasm. Preserve source.zip and binary hashes; reject source mutations during build.",
});
const result = spawnSync("just", ["portable-build"], {
  encoding: "utf8",
  maxBuffer: 32 * 1024 * 1024,
});
await writeFile(join(directory, "build.log"), result.stdout + result.stderr, {
  flag: "wx",
});
assert.equal(result.status, 0, result.stderr);
for (const source of sources)
  assert.equal(digest(await readFile(source.path)), source.sha256, source.path);
const binaries = await Promise.all(
  [
    "crates/idealis-core/target/release/idealis-core",
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
await save("build.json", { status: "complete", binaries });
