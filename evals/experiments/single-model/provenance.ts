import { readFile } from "node:fs/promises";
import { fingerprint } from "../parallel-safety/provenance.js";
import { typeScriptSources } from "../source-files.js";
import { digest } from "../usable/provenance.js";

export const sourceFingerprint = async () => {
  const base = await fingerprint();
  const paths = [
    "evals/plans/0019-single-model-scaling.md",
    "evals/experiments/tlc-throughput/protocol.ts",
    ...(await typeScriptSources(
      "evals/experiments/single-model",
      "tests/single-model",
    )),
  ];
  const sources = [
    ...base.sources,
    ...(await Promise.all(
      paths.map(async (path) => ({
        path,
        sha256: digest(await readFile(path)),
      })),
    )),
  ].sort((left, right) => left.path.localeCompare(right.path));
  return { sources, sourceVersion: digest(JSON.stringify(sources)) };
};
