import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

export const digest = (input: string | Uint8Array): string =>
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
    "crates/ralloy",
    "packages/idealis/src/alloy.ts",
    "packages/idealis/src/contracts.ts",
    "packages/idealis/src/trace.ts",
    "evals/experiments/alloy-instance",
    "tests/alloy-instance",
    "evals/experiments/alloy-subset/corpus.ts",
    "evals/experiments/smt-subset/clients.ts",
    "evals/plans/0035-alloy-instance.md",
    "vite.config.ts",
    "playwright.config.ts",
    "package.json",
    "packages/idealis/package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    "scripts/package.ts",
    "scripts/package-test.ts",
    ".github/workflows/ci.yml",
    ".github/workflows/release.yml",
    "justfile",
  ])
    .toString()
    .split("\0")
    .filter(
      (path) =>
        path &&
        (/\.(rs|ts|toml|lock|json|yaml|yml)$/.test(path) ||
          path === "justfile" ||
          path.endsWith("0035-alloy-instance.md")),
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
export const nativePath = "crates/ralloy/target/release/ralloy";
export const wasmPath =
  "crates/ralloy/target/wasm32-unknown-unknown/release/ralloy.wasm";
