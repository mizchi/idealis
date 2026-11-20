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
    "evals/experiments/sat-foundation",
    "tests/sat-foundation",
    "evals/plans/0037-sat-foundation.md",
    "evals/plans/0037-sat-protocol.md",
    "playwright.config.ts",
    "justfile",
    ".github/workflows/ci.yml",
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
    .filter((path) => path && !path.endsWith("README.md"));
  const sources = await Promise.all(
    [...new Set(paths)]
      .sort()
      .map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  );
  return { sources, sourceVersion: digest(JSON.stringify(sources)) };
};
