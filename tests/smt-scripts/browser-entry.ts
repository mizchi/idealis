import {
  controls,
  generated,
  handCases,
} from "../../evals/experiments/smt-scripts/corpus.js";
import { createWasm } from "../../evals/experiments/smt-subset/wasm.js";
import { solution } from "../../evals/experiments/smt-subset/wire.js";
export const run = async (assetBase = "") => {
  const core = await createWasm(
    new Uint8Array(
      await (await fetch(`${assetBase}/subset.wasm`)).arrayBuffer(),
    ),
  );
  let scripts = 0;
  let checks = 0;
  for (const input of [...handCases(), ...generated(0x4101, 4)]) {
    const result = core.request({
      kind: "run-script",
      source: input.source,
      maxWork: 2_000_000,
    });
    if (
      result.kind !== "script-results" ||
      result.results.length !== input.snapshots.length
    )
      throw new Error(input.name);
    for (const [index, query] of result.results.entries()) {
      if (query.status !== input.snapshots[index]?.expected)
        throw new Error(`${input.name}-${index}`);
      checks++;
    }
    scripts++;
  }
  for (const source of controls)
    if (
      core.request({ kind: "run-script", source, maxWork: 0 }).kind !== "error"
    )
      throw new Error(source);
  const source = "(check-sat)(check-sat)(check-sat)";
  const full = core.request({ kind: "run-script", source, maxWork: 1000 });
  if (full.kind !== "script-results" || !full.results[0])
    throw new Error("Missing results");
  const bounded = core.request({
    kind: "run-script",
    source,
    maxWork: full.results[0].stats.work,
  });
  if (bounded.kind !== "script-results") throw new Error("Missing results");
  const empty = core.request({ kind: "run-script", source: "", maxWork: 0 });
  const handle = core.request({
    kind: "prepare",
    source: "(declare-const x Int)(assert (= x 9))",
  });
  if (handle.kind !== "prepared") throw new Error("Missing handle");
  const invalid = core.request({
    kind: "run-script",
    source: "(check-sat)(pop 1)",
    maxWork: 1000,
  });
  const preserved = solution(
    core.request({ kind: "check", handle: handle.handle, maxWork: 1000 }),
  );
  core.request({ kind: "release", handle: handle.handle });
  return {
    scripts,
    checks,
    bounded: bounded.results.map((value) => value.status),
    empty: empty.kind === "script-results" ? empty.results.length : null,
    error: invalid.kind,
    preserved: preserved.values.x,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
