import { createHash } from "node:crypto";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

export const runtimeFingerprint = async () => {
  const collect = async (directory: string): Promise<string[]> =>
    (
      await Promise.all(
        (await readdir(directory, { withFileTypes: true })).map(async (entry) =>
          entry.isDirectory()
            ? collect(join(directory, entry.name))
            : entry.isFile() && /\.(ts|rs)$/.test(entry.name)
              ? [join(directory, entry.name)]
              : [],
        ),
      )
    ).flat();
  const files = [
    ...(
      await Promise.all(
        [
          "packages/idealis/src",
          "examples",
          "evals/experiments/harness",
          "evals/experiments/runtime",
          "tests/runtime",
          "evals/experiments/rust-probe/src",
        ].map(collect),
      )
    ).flat(),
    "justfile",
    "package.json",
    "packages/idealis/package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    "tsconfig.json",
    "packages/idealis/tsconfig.json",
    "evals/experiments/rust-probe/Cargo.toml",
    "evals/experiments/rust-probe/Cargo.lock",
  ].sort();
  const sources = await Promise.all(
    files.map(async (path) => ({
      path,
      sha256: createHash("sha256")
        .update(await readFile(path))
        .digest("hex"),
    })),
  );
  return {
    sourceVersion: createHash("sha256")
      .update(JSON.stringify(sources))
      .digest("hex"),
    sources,
  };
};
