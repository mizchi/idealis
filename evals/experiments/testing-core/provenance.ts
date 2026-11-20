import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
export const digest = (value: string | Uint8Array): string =>
  createHash("sha256").update(value).digest("hex");
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
    "crates/idealis-connect",
    "crates/idealis-cli",
    "packages/idealis/src/testing.ts",
    "packages/idealis/src/testing-wire.ts",
    "packages/idealis/src/contracts.ts",
    "packages/idealis/src/conformance.ts",
    "packages/idealis/src/shrink.ts",
    "packages/idealis/src/trace.ts",
    "examples/durable-counter.ts",
    "examples/counter-model.ts",
    "evals/experiments/testing-core",
    "tests/testing-core",
    "evals/experiments/smt-subset/clients.ts",
    "evals/plans/0036-testing-core.md",
    "playwright.config.ts",
    "vite.config.ts",
    "package.json",
    "packages/idealis/package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    "scripts/package.ts",
    "scripts/package-test.ts",
    "scripts/release.ts",
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
          path.endsWith("0036-testing-core.md")),
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
