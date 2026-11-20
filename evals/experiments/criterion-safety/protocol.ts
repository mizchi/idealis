import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { sourceFingerprint } from "../single-model/provenance.js";
import { typeScriptSources } from "../source-files.js";
import { digest } from "../usable/provenance.js";

export const routes = [
  { name: "before-native-1", stage: "before", backend: "native", workers: 1 },
  { name: "after-native-1", stage: "after", backend: "native", workers: 1 },
  { name: "before-native-8", stage: "before", backend: "native", workers: 8 },
  { name: "after-native-8", stage: "after", backend: "native", workers: 8 },
  {
    name: "before-wasm-1",
    stage: "before",
    backend: "wasm",
    workers: undefined,
  },
  { name: "after-wasm-1", stage: "after", backend: "wasm", workers: undefined },
] as const;
export const binaries = async () => {
  const paths = [
    {
      stage: "before",
      backend: "native",
      path: ".artifacts/criterion-safety/before/idealis-core",
    },
    {
      stage: "before",
      backend: "wasm",
      path: ".artifacts/criterion-safety/before/idealis_core.wasm",
    },
    {
      stage: "after",
      backend: "native",
      path: "crates/idealis-core/target/release/idealis-core",
    },
    {
      stage: "after",
      backend: "wasm",
      path: "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
    },
  ];
  const result = await Promise.all(
    paths.map(async (entry) => ({
      ...entry,
      sha256: digest(await readFile(entry.path)),
    })),
  );
  const original = JSON.parse(
    await readFile("evals/runs/0022-readme-comparison/manifest.json", "utf8"),
  );
  for (const entry of result.filter((entry) => entry.stage === "before"))
    assert.ok(
      original.environment.binaries.some(
        (binary: { sha256: string }) => binary.sha256 === entry.sha256,
      ),
      "Before binaries must match the published 0022 run",
    );
  return result;
};
export const fingerprint = async () => {
  const base = await sourceFingerprint();
  const paths = [
    "evals/plans/0023-criterion-safety.md",
    "evals/plans/0023-adaptive-policy.md",
    ...(await typeScriptSources(
      "evals/experiments/criterion-safety",
      "tests/criterion-safety",
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
