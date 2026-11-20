import { readdir, readFile } from "node:fs/promises";
import { sourceFingerprint as frontierFingerprint } from "../frontier-safety/provenance.js";
import { typeScriptSources } from "../source-files.js";
import { digest } from "../usable/provenance.js";

export const sourceFingerprint = async () => {
  const base = await frontierFingerprint();
  const paths = [
    "evals/plans/0022-readme-comparison.md",
    "node_modules/tla-ts/package.json",
    ...(await readdir("node_modules/tla-ts/dist"))
      .filter((path) => path.endsWith(".js"))
      .map((path) => `node_modules/tla-ts/dist/${path}`),
    ...(await typeScriptSources(
      "evals/experiments/readme-comparison",
      "tests/readme-comparison",
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
