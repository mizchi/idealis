import { createWasm } from "../../evals/experiments/smt-subset/wasm.js";
import type { CheckCache } from "../../evals/experiments/smt-subset/wire.js";

const buildDigest =
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
export const run = async (assetBase = "") => {
  const core = await createWasm(
    new Uint8Array(
      await (await fetch(`${assetBase}/subset.wasm`)).arrayBuffer(),
    ),
  );
  const source =
    "(declare-const x Int)(declare-const b Bool)(assert (= x (ite b 3 4)))(check-sat-assuming (b))(check-sat-assuming ((not b)))(check-sat-assuming (b))";
  const runScript = (maxWork: number, enabled: boolean, cache?: CheckCache) => {
    const reply = core.request({
      kind: "run-script-incremental",
      source,
      maxWork,
      enabled,
      buildDigest,
      maxEntries: 8,
      ...(cache ? { cache } : {}),
    });
    if (reply.kind !== "incremental-script-results")
      throw new Error(JSON.stringify(reply));
    return reply;
  };
  const cold = runScript(100_000, true);
  const warm = runScript(0, true, cold.cache);
  const disabled = runScript(0, false, cold.cache);
  return {
    cold: cold.stats,
    warm: warm.stats,
    values: warm.results.map((result) => result.values.x),
    work: warm.results.map((result) => result.stats.work),
    disabled: disabled.results.map((result) => result.status),
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
