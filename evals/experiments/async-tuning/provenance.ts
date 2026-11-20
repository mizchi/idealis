import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { digest } from "../usable/provenance.js";

export const fingerprint = async () => {
  const files = execFileSync("git", [
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
    "--",
    "crates/roracle",
    "evals/experiments/workspace-oracle",
    "tests/workspace-oracle",
    "fixtures/workspace-oracle",
    "packages/idealis/src",
    "scripts",
    "tests",
    "examples",
    "adapters/go",
    "crates/idealis-core",
    "crates/rtlc",
    "crates/rtla",
    "crates/rtla-compiler",
    "evals/experiments/async-tuning",
    "tests/async-tuning",
    "evals/experiments/harness",
    "evals/experiments/cross-language",
    "tests/cross-language",
    "evals/experiments/shared-oracle-rust",
    "evals/experiments/usable/mutex.ts",
    "evals/experiments/usable/provenance.ts",
    "fixtures",
    "evals/plans/0008-async-tuning.md",
    "justfile",
    ".github/workflows/ci.yml",
    "package.json",
    "packages/idealis/package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    "tsconfig.json",
    "packages/idealis/tsconfig.json",
  ])
    .toString()
    .split("\0")
    .filter(Boolean);
  const sources = await Promise.all(
    [
      ...new Set([
        ...files,
        "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
      ]),
    ]
      .sort()
      .map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  );
  return { sources, sourceVersion: digest(JSON.stringify(sources)) };
};
