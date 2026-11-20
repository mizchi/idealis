import { readFile } from "node:fs/promises";
import { fingerprint } from "../rust-safety/provenance.js";
import { typeScriptSources } from "../source-files.js";
import { digest } from "../usable/provenance.js";

export const symmetryFingerprint = async () => {
  const base = await fingerprint();
  const paths = [
    "evals/plans/0060-restricted-symmetry.md",
    "evals/plans/0061-symmetry-next.md",
    ...(await typeScriptSources(
      "evals/experiments/symmetry",
      "tests/symmetry",
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
