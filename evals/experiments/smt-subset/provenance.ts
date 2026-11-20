import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
export const digest = (input: Uint8Array | string): string =>
  createHash("sha256").update(input).digest("hex");
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
    "evals/experiments/smt-subset",
    "tests/smt-subset",
    "crates/rusmt",
    "evals/plans/0030-smt-subset.md",
    "evals/plans/0031-smt-reuse.md",
    "evals/plans/0032-rsmt-extraction.md",
    "evals/experiments/harness/ttl.ts",
    "packages/idealis/src/conformance.ts",
    "packages/idealis/src/contracts.ts",
    "packages/idealis/src/trace.ts",
    "examples/counter-model.ts",
    "examples/durable-counter.ts",
    "playwright.config.ts",
    "justfile",
    "package.json",
    "packages/idealis/package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    ".github/workflows/ci.yml",
  ])
    .toString()
    .split("\0")
    .filter(
      (path) =>
        path &&
        (path.endsWith(".ts") ||
          path.endsWith(".rs") ||
          path.endsWith(".toml") ||
          path.endsWith(".lock") ||
          path.endsWith(".yaml") ||
          path.endsWith(".yml") ||
          path.endsWith(".json") ||
          path.endsWith("0030-smt-subset.md") ||
          path.endsWith("0031-smt-reuse.md") ||
          path.endsWith("0032-rsmt-extraction.md") ||
          path === "justfile"),
    );
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
