import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { createSqliteCheckCacheStore } from "../../../packages/idealis/src/check-cache-node.js";
import { createCheckCache } from "../../../packages/idealis/src/check-cache.js";
import { names } from "../quint-generated/cases.js";
import { type CacheModelInput, runCachedModels } from "./scenarios.js";

const [output, saved] = process.argv.slice(2);
if (!output) throw new Error("Usage: cache-eval output [saved]");
const directory = resolve(output);
await mkdir(directory);
const digest = (value: Uint8Array | string) =>
  createHash("sha256").update(value).digest("hex");
const bytes = await readFile(
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
);
const models: CacheModelInput[] = saved
  ? JSON.parse(await readFile(resolve(saved, "models.json"), "utf8"))
  : await Promise.all(
      names.map(async (name) => ({
        name,
        source: await readFile(
          `fixtures/quint-generated/generated/${name}.tla`,
          "utf8",
        ),
      })),
    );
assert.deepEqual(
  models.map((input) => input.name),
  [...names],
);
const oracle = saved
  ? await readFile(resolve(saved, "tlc-oracle.json"), "utf8")
  : await readFile("fixtures/quint-generated/tlc-oracle.json", "utf8");
const sourcePaths = [
  "packages/idealis/src/check-cache.ts",
  "packages/idealis/src/check-cache-wire.ts",
  "packages/idealis/src/check-cache-policy.ts",
  "packages/idealis/src/check-cache-node.ts",
  "packages/idealis/src/check-cache-http.ts",
  "packages/idealis/src/cached-scheduling.ts",
  "packages/idealis/src/backend.ts",
  "packages/idealis/src/contracts.ts",
  "evals/experiments/verified-cache/scenarios.ts",
  "evals/experiments/verified-cache/run.ts",
];
const sources = await Promise.all(
  sourcePaths.map(async (path) => ({
    path,
    sha256: digest(await readFile(path)),
  })),
);
const manifest = {
  plan: "evals/plans/verified-cache.md",
  sources,
  binaryDigest: digest(bytes),
  node: process.version,
  modelsDigest: digest(JSON.stringify(models)),
  oracleDigest: digest(oracle),
  saved: saved ?? null,
};
await writeFile(
  resolve(directory, "manifest.json"),
  JSON.stringify(manifest, null, 2),
);
await writeFile(
  resolve(directory, "models.json"),
  JSON.stringify(models, null, 2),
);
await writeFile(resolve(directory, "tlc-oracle.json"), oracle);
if (saved) {
  const before = JSON.parse(
    await readFile(resolve(saved, "manifest.json"), "utf8"),
  );
  assert.equal(manifest.binaryDigest, before.binaryDigest);
  assert.deepEqual(manifest.sources, before.sources);
  assert.equal(manifest.modelsDigest, before.modelsDigest);
  assert.equal(manifest.oracleDigest, before.oracleDigest);
}
try {
  const path = resolve(directory, "cache.db");
  const first = createSqliteCheckCacheStore(path);
  const cold = await runCachedModels(
    bytes,
    models,
    createCheckCache(first),
    "cold",
  ).finally(() => first.close());
  const second = createSqliteCheckCacheStore(path);
  const warm = await runCachedModels(
    bytes,
    models,
    createCheckCache(second),
    "warm",
  ).finally(() => second.close());
  await writeFile(
    resolve(directory, "observations.json"),
    JSON.stringify({ cold, warm }, null, 2),
  );
  assert.equal(cold.calls, 6);
  assert.equal(warm.calls, 0);
  const reference = JSON.parse(oracle);
  for (const [index, row] of cold.rows.entries()) {
    const expected = reference.find(
      (entry: { name: string; property: string }) =>
        entry.name === row.name && entry.property === "safety",
    );
    assert.ok(expected);
    assert.equal(row.kind, "executed");
    assert.equal(row.outcome.kind, "explicit-complete");
    assert.equal(row.states, expected.states);
    assert.equal(warm.rows[index]?.kind, "cached");
    assert.deepEqual(warm.rows[index]?.outcome, row.outcome);
  }
  const result = {
    status: "complete",
    models: models.length,
    coldCheckCalls: cold.calls,
    warmCheckCalls: warm.calls,
    states: cold.rows.map((row) => row.states),
    elapsedMs: {
      cold: cold.rows.reduce((sum, row) => sum + row.elapsedMs, 0),
      warm: warm.rows.reduce((sum, row) => sum + row.elapsedMs, 0),
    },
    timingScope:
      "six sequential task-cache lookups/executions; compilation and Wasm initialization excluded; descriptive, no speedup claim",
  };
  if (saved) {
    const before = JSON.parse(
      await readFile(resolve(saved, "result.json"), "utf8"),
    );
    for (const key of [
      "status",
      "models",
      "coldCheckCalls",
      "warmCheckCalls",
      "states",
    ] as const)
      assert.deepEqual(result[key], before[key]);
  }
  await writeFile(
    resolve(directory, "result.json"),
    JSON.stringify(result, null, 2),
  );
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  await writeFile(
    resolve(directory, "error.json"),
    JSON.stringify({ message: String(error) }),
  );
  throw error;
}
