import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

export const digest = (value: string | Uint8Array): string =>
  createHash("sha256").update(value).digest("hex");
export const portableFingerprint = async () => {
  const files = [
    ...new Set(
      execFileSync("git", [
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
        "packages/idealis/src",
        "examples",
        "evals/experiments/portable",
        "tests/portable",
        "crates/idealis-core",
        "crates/rtlc",
        "crates/rtla",
        "crates/rtla-compiler",
        "fixtures",
        "tests",
        "justfile",
        "package.json",
        "packages/idealis/package.json",
        "pnpm-workspace.yaml",
        "pnpm-lock.yaml",
        "tsconfig.json",
        "packages/idealis/tsconfig.json",
        "evals/plans/0004-portable-core.md",
      ])
        .toString()
        .split("\0")
        .filter(Boolean),
    ),
  ].sort();
  const sources = await Promise.all(
    files.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  );
  return { sources, sourceVersion: digest(JSON.stringify(sources)) };
};
