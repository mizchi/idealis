import { createRtlcBackend } from "../../../packages/idealis/src/backend.js";
import { runCachedCheck } from "../../../packages/idealis/src/check-cache.js";
import type {
  CheckCache,
  CheckCacheJob,
  PortableCore,
} from "../../../packages/idealis/src/contracts.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import { bounds, config } from "../quint-generated/cases.js";

export interface CacheModelInput {
  readonly name: string;
  readonly source: string;
}
const digest = async (value: Uint8Array) =>
  Array.from(
    new Uint8Array(
      await crypto.subtle.digest("SHA-256", Uint8Array.from(value)),
    ),
  )
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");

export const runCachedModels = async (
  bytes: Uint8Array,
  models: readonly CacheModelInput[],
  cache: CheckCache,
  runId: string,
) => {
  const binaryDigest = await digest(bytes);
  const wasm = await createPortableWasm(bytes);
  const calls: string[] = [];
  const core: PortableCore = {
    request: (command) => {
      if (command.request.kind === "check-safety")
        calls.push(command.request.kind);
      return wasm.request(command);
    },
  };
  const backend = createRtlcBackend(core, { version: binaryDigest });
  const rows = [];
  for (const input of models) {
    const job: CheckCacheJob = {
      buildDigest: binaryDigest,
      backend: backend.descriptor,
      request: {
        format: "modelt.backend-request",
        version: 1,
        scope: {
          modelId: await digest(new TextEncoder().encode(input.source)),
          propertyId: "safety",
          semantics: "tlc-1.7.4-int32",
          assumptions: [],
        },
        query: { kind: "explicit", bounds },
        input: compileTlaSource(core, {
          source: input.source,
          config: config("safety"),
          irVersion: 2,
        }),
      },
    };
    const started = performance.now();
    const result = await runCachedCheck(cache, job, backend, {
      owner: runId,
      runId,
      leaseMs: 60000,
      now: Date.now,
    });
    if (result.kind === "deferred")
      throw new Error("Unexpected concurrent producer");
    const output =
      result.kind === "cached" ? result.artifact.result : result.result;
    rows.push({
      name: input.name,
      kind: result.kind,
      outcome: output.outcome,
      states: output.progress?.states,
      elapsedMs: performance.now() - started,
    });
  }
  return { binaryDigest, calls: calls.length, rows };
};
