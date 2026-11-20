import { createAlloyWasm } from "../../packages/idealis/src/alloy.js";

export const run = async (assetBase = "") => {
  const runtime = await createAlloyWasm(
    new Uint8Array(
      await (await fetch(`${assetBase}/ralloy.wasm`)).arrayBuffer(),
    ),
  );
  const input = {
    source:
      "sig N { edge: set N } fact { no iden & edge } assert Safe { no iden & ^edge } check Safe for 2 but exactly 2 N, 0 Int",
    command: 0,
    maxWork: 100_000,
  };
  const found = runtime.analyze(input);
  if (!found.instance) throw new Error("Missing counterexample");
  return {
    status: found.status,
    check: runtime.checkInstance(input, found.instance).status,
    dag: runtime.checkInstance(input, {
      signatures: { N: ["a", "b"] },
      fields: { edge: [["a", "b"]] },
    }).status,
    unknown: runtime.checkInstance({ ...input, maxWork: 0 }, found.instance)
      .status,
    invalid: runtime.checkInstance(input, {
      signatures: { N: ["a", "b"] },
      fields: { edge: [["a", "a"]] },
    }).status,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
