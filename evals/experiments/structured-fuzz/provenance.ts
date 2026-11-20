import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { sources } from "./corpus.js";
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
    "crates/idealis-connect",
    "evals/experiments/structured-fuzz",
    "tests/structured-fuzz",
    "evals/experiments/smt-subset/clients.ts",
    "evals/plans/0039-structured-fuzz.md",
    "playwright.config.ts",
    "justfile",
    ".github/workflows/ci.yml",
    ".github/workflows/fuzz.yml",
    "package.json",
    "packages/idealis/package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    "tsconfig.json",
    "packages/idealis/tsconfig.json",
    "vite.config.ts",
    ".gitignore",
  ])
    .toString()
    .split("\0")
    .filter(
      (path) =>
        path && !path.endsWith("README.md") && !path.endsWith("LICENSE"),
    );
  const files = await Promise.all(
    [...new Set([...paths, ...sources])]
      .sort()
      .map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  );
  return { sources: files, sourceVersion: digest(JSON.stringify(files)) };
};
