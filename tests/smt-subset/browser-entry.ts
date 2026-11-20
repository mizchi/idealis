import { createWasm } from "../../evals/experiments/smt-subset/wasm.js";
import { solution } from "../../evals/experiments/smt-subset/wire.js";
export const run = async (assetBase = "") => {
  const core = await createWasm(
    new Uint8Array(
      await (await fetch(`${assetBase}/subset.wasm`)).arrayBuffer(),
    ),
  );
  const source =
    "(declare-const amount Int)(declare-const other Int)(assert (and (= other 3)(= (+ amount other) 10)))";
  const sat = solution(core.request({ kind: "solve", source, maxWork: 10000 }));
  const unsat = solution(
    core.request({
      kind: "solve",
      source:
        "(declare-const x Int)(declare-const y Int)(assert (< x y))(assert (<= y x))",
      maxWork: 10000,
    }),
  );
  const unknown = solution(core.request({ kind: "solve", source, maxWork: 0 }));
  const error = core.request({
    kind: "solve",
    source: "(declare-const x Real)",
    maxWork: 10000,
  });
  const repeated = solution(
    core.request({ kind: "solve", source, maxWork: 10000 }),
  );
  return {
    sat: sat.status,
    amount: sat.values.amount,
    unsat: unsat.status,
    unknown: unknown.status,
    error: error.kind,
    repeated: repeated.values.amount,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
