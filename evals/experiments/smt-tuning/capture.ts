import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { digest, fingerprint } from "./provenance.js";

const binary = process.argv[2];
const output = process.argv[3];
assert.ok(binary && output);
const source = await fingerprint();
await writeFile(
  output,
  JSON.stringify(
    {
      ...source,
      binary: resolve(binary),
      sha256: digest(await readFile(binary)),
      benchmarkSources: source.sources.filter((file) =>
        file.path.startsWith("crates/rusmt/benches/"),
      ),
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      dirtyPaths: execFileSync("git", ["status", "--short"]).toString(),
      rustc: execFileSync("rustc", ["--version"]).toString().trim(),
      build:
        "cargo bench --no-run --bench solver --locked; CARGO_PROFILE_BENCH_STRIP=none; other bench defaults unchanged",
    },
    null,
    2,
  ),
  { flag: "wx" },
);
