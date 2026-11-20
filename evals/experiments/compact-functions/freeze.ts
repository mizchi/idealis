import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { digest } from "../usable/provenance.js";

const output = process.argv[2];
const kernelOnly = process.argv[3] === "kernel";
assert.ok(process.argv[3] === undefined || kernelOnly);
assert.ok(output);
await mkdir(output);
for (const [source, name] of kernelOnly
  ? []
  : ([
      ["crates/idealis-core/target/release/idealis-core", "idealis-core"],
      [
        "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
        "idealis_core.wasm",
      ],
    ] as const))
  await writeFile(join(output, name), await readFile(source), {
    flag: "wx",
    mode: name === "idealis-core" ? 0o755 : 0o644,
  });
const entries = await readdir("crates/rtlc/target/release/deps");
const executables = entries.filter((name) => /^safety-[a-f0-9]+$/.test(name));
assert.equal(executables.length, 1, "Use an unambiguous safety bench build");
const executable = executables[0];
assert.ok(executable);
const binary = await readFile(
  join("crates/rtlc/target/release/deps", executable),
);
await writeFile(join(output, "safety-bench"), binary, {
  flag: "wx",
  mode: 0o755,
});
const paths = execFileSync("rg", [
  "--files",
  "crates/rtlc",
  "crates/rtla",
  "crates/rtla-compiler",
  "crates/idealis-core",
])
  .toString()
  .trim()
  .split("\n")
  .sort();
execFileSync("tar", ["-cf", resolve(output, "source.tar"), "--", ...paths]);
await writeFile(
  join(output, "criterion.json"),
  `${JSON.stringify({ binarySha256: digest(binary), benchSha256: digest(await readFile("crates/rtlc/src/benchmarks.rs")) }, null, 2)}\n`,
  { flag: "wx" },
);
const files = await Promise.all(
  (await readdir(output)).map(async (path) => ({
    path,
    sha256: digest(await readFile(join(output, path))),
  })),
);
await writeFile(
  join(output, "manifest.json"),
  `${JSON.stringify({ plan: process.env.IDEALIS_COMPACT_PLAN ?? "0056", revision: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(), files, sources: await Promise.all(paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) }))) }, null, 2)}\n`,
  { flag: "wx" },
);
