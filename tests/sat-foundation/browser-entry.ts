import { createCorpus } from "../../evals/experiments/sat-foundation/corpus.js";
import {
  enumerate,
  validate,
} from "../../evals/experiments/sat-foundation/oracle.js";
import { createWasm } from "../../evals/experiments/sat-foundation/wasm.js";

export const run = async (assetBase = "") => {
  const bytes = new Uint8Array(
    await (await fetch(`${assetBase}/sat.wasm`)).arrayBuffer(),
  );
  const wasm = await createWasm(bytes);
  const records = createCorpus(0x3701, 32).map((input) => {
    const reply = wasm.request(input);
    return {
      input,
      reply,
      expected: enumerate(input),
      statuses: validate(input, reply),
    };
  });
  return {
    records,
    imports: wasm.imports,
    bytes: bytes.length,
    isolated: globalThis.crossOriginIsolated ?? false,
    budget: wasm.request({
      variables: 1,
      clauses: [],
      rounds: [{ add: [], assumptions: [] }],
      maxWork: 0,
    }),
  };
};
