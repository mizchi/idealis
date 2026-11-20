import { createWasm } from "../../evals/experiments/alloy-subset/wasm.js";
import { analysis } from "../../evals/experiments/alloy-subset/wire.js";
export const run = async (assetBase = "") => {
  const core = await createWasm(
    new Uint8Array(
      await (await fetch(`${assetBase}/ralloy.wasm`)).arrayBuffer(),
    ),
  );
  const source =
    "sig N { edge: set N } fact { no iden & edge } assert P { no iden & ^edge } check P for 2 but exactly 2 N, 0 Int";
  const result = analysis(
    core.request({ kind: "analyze", source, command: 0, maxWork: 2_000_000 }),
  );
  const unknown = analysis(
    core.request({ kind: "analyze", source, command: 0, maxWork: 0 }),
  );
  const rejected = core.request({
    kind: "analyze",
    source: "var sig N {} run {}",
    command: 0,
    maxWork: 1000,
  });
  return {
    status: result.status,
    edges: result.instance?.fields.edge?.length,
    unknown: unknown.status,
    rejected: rejected.kind,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
