import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

const path = "packages/idealis/src/generated/roracle.ts";
const raw = execFileSync(
  "cargo",
  [
    "run",
    "--quiet",
    "--locked",
    "--manifest-path",
    "crates/roracle/Cargo.toml",
    "--features",
    "typescript",
    "--example",
    "typescript",
  ],
  { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
);
const formatted = execFileSync(
  "pnpm",
  ["exec", "vp", "fmt", "--stdin-filepath", path],
  { input: raw, encoding: "utf8" },
);
if (process.argv.includes("--check"))
  assert.equal(
    await readFile(path, "utf8"),
    formatted,
    "Generated oracle types are stale; run just oracle-types",
  );
else {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, formatted);
}
