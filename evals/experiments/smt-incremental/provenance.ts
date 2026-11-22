import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { digest } from "../smt-subset/provenance.js";

export { digest } from "../smt-subset/provenance.js";
export const fingerprint = async () => {
  const paths = execFileSync("git", [
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
    "crates/rusmt",
    "evals/experiments/smt-subset",
    "tests/smt-subset",
    "evals/experiments/smt-expansion",
    "tests/smt-expansion",
    "evals/experiments/smt-scripts",
    "tests/smt-scripts",
    "evals/experiments/smt-tuning",
    "tests/smt-tuning",
    "evals/experiments/smt-cache",
    "tests/smt-cache",
    "evals/experiments/smt-incremental",
    "tests/smt-incremental",
    "evals/plans/0074-smt-incremental-cache.md",
    "evals/plans/0074a-smt-lazy-cache-keys.md",
    "evals/plans/0074b-smt-memoized-cache-keys.md",
    "evals/plans/0040-smt-expansion.md",
    "evals/plans/0040a-smt-expansion-adjustment.md",
    "evals/plans/0041-smt-scripts.md",
    "evals/plans/0042-smt-tuning.md",
    "evals/plans/0042a-smt-measurement-recheck.md",
    "evals/experiments/profile-tuning/hotspots.ts",
    "packages/idealis/src/trace.ts",
    "packages/idealis/src/conformance.ts",
    "packages/idealis/src/contracts.ts",
    "evals/experiments/harness/ttl.ts",
    "examples/counter-model.ts",
    "examples/durable-counter.ts",
    "scripts/package.ts",
    "scripts/package-test.ts",
    "playwright.config.ts",
    "justfile",
    "package.json",
    "packages/idealis/package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    "tsconfig.json",
    "packages/idealis/tsconfig.json",
    "vite.config.ts",
    ".github/workflows/ci.yml",
  ])
    .toString()
    .split("\0")
    .filter((path) => path && !path.endsWith("README.md"));
  const sources = await Promise.all(
    [
      ...new Set([
        ...paths,
        "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
      ]),
    ]
      .sort()
      .map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  );
  return { sources, sourceVersion: digest(JSON.stringify(sources)) };
};
