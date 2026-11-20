import { bounds, config } from "../evals/experiments/quint-generated/cases.js";
import {
  type CacheModelInput,
  runCachedModels,
} from "../evals/experiments/verified-cache/scenarios.js";
import { createHttpCheckCacheStore } from "../packages/idealis/src/check-cache-http.js";
import { createCheckCache } from "../packages/idealis/src/check-cache.js";
import type { SafetyExpansionCache } from "../packages/idealis/src/contracts.js";
import { createPortableWasm } from "../packages/idealis/src/portable-wasm.js";
import {
  checkSafetyIncremental,
  createSafetyExpansionCache,
} from "../packages/idealis/src/safety-expansions.js";
import { compileTlaSource } from "../packages/idealis/src/tla-source.js";

export const run = async (
  endpoint: string,
  models: readonly CacheModelInput[],
  runId: string,
) => {
  const bytes = new Uint8Array(await (await fetch("/core.wasm")).arrayBuffer());
  return runCachedModels(
    bytes,
    models,
    createCheckCache(createHttpCheckCacheStore(endpoint)),
    runId,
  );
};

export const runExpansions = async (
  models: readonly CacheModelInput[],
  snapshots: readonly SafetyExpansionCache[] = [],
) => {
  const bytes = new Uint8Array(await (await fetch("/core.wasm")).arrayBuffer());
  const buildDigest = Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (byte) => byte.toString(16).padStart(2, "0"),
  ).join("");
  const core = await createPortableWasm(bytes);
  return models.map((input, index) => {
    const program = compileTlaSource(core, {
      source: input.source,
      config: config("safety"),
      irVersion: 2,
    });
    const normal = checkSafetyIncremental(core, program, bounds, {
      enabled: true,
      cache: snapshots[index] ?? createSafetyExpansionCache(),
      buildDigest,
      maxEntries: 10000,
    });
    const changed = checkSafetyIncremental(
      core,
      compileTlaSource(core, {
        source: input.source,
        config: config("broken"),
        irVersion: 2,
      }),
      bounds,
      { enabled: true, cache: normal.cache, buildDigest, maxEntries: 10000 },
    );
    return { name: input.name, normal, changed };
  });
};
