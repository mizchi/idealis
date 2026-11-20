import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  copyFile,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { resolve } from "node:path";
import { nativePath } from "../evals/experiments/roracle-native/client.js";

await mkdir(".artifacts", { recursive: true });
const output = await mkdtemp(resolve(".artifacts/roracle-native-build-"));
execFileSync(
  "pnpm",
  [
    "exec",
    "napi",
    "build",
    "--manifest-path",
    "evals/experiments/roracle-native/rust/Cargo.toml",
    "--package-json-path",
    "evals/experiments/roracle-native/package.json",
    "--output-dir",
    output,
    "--dts",
    "index.d.ts",
    "--release",
    "--",
    "--locked",
  ],
  { stdio: "inherit" },
);
const declaration = "evals/experiments/roracle-native/generated/index.d.ts";
const formatted = execFileSync(
  "pnpm",
  ["exec", "vp", "fmt", "--stdin-filepath", declaration],
  { input: await readFile(resolve(output, "index.d.ts")), encoding: "utf8" },
);
if (process.argv.includes("--check"))
  assert.equal(
    await readFile(declaration, "utf8"),
    formatted,
    "Native declarations are stale; regenerate with scripts/build-native-oracle.ts",
  );
else await writeFile(declaration, formatted);
const binaries = (await readdir(output)).filter((name) =>
  name.endsWith(".node"),
);
assert.equal(binaries.length, 1);
const binary = binaries[0];
assert.ok(binary);
await copyFile(resolve(output, binary), nativePath);
