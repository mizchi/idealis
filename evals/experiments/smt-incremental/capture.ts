import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { isBench } from "./cases.js";
import { digest, fingerprint } from "./provenance.js";

// capture <bench-binary> <solver|cache|protocol> <library-commit> <output.json>
const [binary, bench, library, output] = process.argv.slice(2);
assert.ok(binary && isBench(bench) && library && output);
const source = await fingerprint();
await writeFile(
  output,
  JSON.stringify(
    {
      ...source,
      bench,
      library,
      binary: resolve(binary),
      sha256: digest(await readFile(binary)),
      benchmarkSources: source.sources.filter((file) =>
        file.path.startsWith("crates/rusmt/benches/"),
      ),
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      dirtyPaths: execFileSync("git", ["status", "--short"]).toString(),
      rustc: execFileSync("rustc", ["--version"]).toString().trim(),
      build: `CARGO_PROFILE_BENCH_STRIP=none cargo bench --no-run --locked --bench ${bench}`,
    },
    null,
    2,
  ),
  { flag: "wx" },
);
