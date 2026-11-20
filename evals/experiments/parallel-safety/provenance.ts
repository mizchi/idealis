import { readFile } from "node:fs/promises";
import { sourceFingerprint } from "../checker-benchmark/provenance.js";
import { typeScriptSources } from "../source-files.js";
import { digest } from "../usable/provenance.js";

export const fingerprint = async () => {
  const source = await sourceFingerprint();
  const paths = [
    "evals/plans/0017-parallel-safety.md",
    "evals/experiments/usable/native.ts",
    "evals/experiments/usable/provenance.ts",
    "playwright.config.ts",
    ...(await typeScriptSources(
      "evals/experiments/parallel-safety",
      "tests/parallel-safety",
    )),
  ];
  const sources = [
    ...source.sources,
    ...(await Promise.all(
      paths.map(async (path) => ({
        path,
        sha256: digest(await readFile(path)),
      })),
    )),
  ].sort((left, right) => left.path.localeCompare(right.path));
  return { sources, sourceVersion: digest(JSON.stringify(sources)) };
};
