import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { digest } from "../usable/provenance.js";

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
    "packages/idealis/src",
    "scripts",
    "tests",
    "crates/idealis-core",
    "crates/rtlc",
    "crates/rtla",
    "crates/rtla-compiler",
    "evals/experiments/rust-safety",
    "tests/rust-safety",
    "evals/experiments/tlc-differential",
    "tests/tlc-differential",
    "evals/plans/0015-tlc-differential.md",
    "evals/experiments/collections-temporal",
    "tests/collections-temporal",
    "fixtures/channel",
    "fixtures/tlc-strings",
    "fixtures/tlc-case",
    "fixtures/tlc-apalache-folds",
    "evals/plans/0012-collections-temporal.md",
    "evals/plans/0013-scalar-comparison.md",
    "evals/plans/0014-value-hash.md",
    "fixtures/die-hard",
    "evals/plans/0009-rust-safety.md",
    "evals/plans/0010-rust-safety-storage.md",
    "evals/plans/0011-rust-safety-arena.md",
    "justfile",
    "package.json",
    "packages/idealis/package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    "tsconfig.json",
    "packages/idealis/tsconfig.json",
    ".github/workflows/ci.yml",
  ])
    .toString()
    .split("\0")
    .filter(Boolean);
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
