import {
  controls,
  generated,
  handCases,
} from "../../evals/experiments/smt-expansion/corpus.js";
import { createWasm } from "../../evals/experiments/smt-subset/wasm.js";
import { solution } from "../../evals/experiments/smt-subset/wire.js";
export const run = async (assetBase = "") => {
  const core = await createWasm(
    new Uint8Array(
      await (await fetch(`${assetBase}/subset.wasm`)).arrayBuffer(),
    ),
  );
  let checked = 0;
  for (const input of [...handCases(), ...generated(0x4001, 24)]) {
    const result = solution(
      core.request({ kind: "solve", source: input.source, maxWork: 2_000_000 }),
    );
    if (result.status !== input.expected) throw new Error(input.name);
    checked++;
  }
  for (const source of controls)
    if (
      core.request({ kind: "solve", source, maxWork: 2_000_000 }).kind !==
      "error"
    )
      throw new Error(source);
  const handle = core.request({
    kind: "prepare",
    source: "(declare-const x Int)(declare-const b Bool)(check-sat)",
  });
  if (handle.kind !== "prepared") throw new Error("prepare");
  const id = handle.handle;
  if (core.request({ kind: "push", handle: id, count: 1 }).kind !== "updated")
    throw new Error("push");
  if (
    core.request({
      kind: "assert",
      handle: id,
      expression: "(= (* 2 x) (ite b 6 8))",
    }).kind !== "updated"
  )
    throw new Error("assert");
  const positive = solution(
    core.request({
      kind: "check",
      handle: id,
      maxWork: 10000,
      assumptions: [{ symbol: "b", positive: true }],
    }),
  );
  const negative = solution(
    core.request({
      kind: "check",
      handle: id,
      maxWork: 10000,
      assumptions: [{ symbol: "b", positive: false }],
    }),
  );
  const error = core.request({
    kind: "assert",
    handle: id,
    expression: "true)(assert false",
  });
  const afterError = solution(
    core.request({
      kind: "check",
      handle: id,
      maxWork: 10000,
      assumptions: [{ symbol: "b", positive: true }],
    }),
  );
  const unknown = solution(
    core.request({ kind: "check", handle: id, maxWork: 0 }),
  );
  if (core.request({ kind: "pop", handle: id, count: 1 }).kind !== "updated")
    throw new Error("pop");
  core.request({ kind: "assert", handle: id, expression: "(= x 9)" });
  const restored = solution(
    core.request({ kind: "check", handle: id, maxWork: 10000 }),
  );
  core.request({ kind: "release", handle: id });
  return {
    checked,
    positive: positive.values.x,
    negative: negative.values.x,
    error: error.kind,
    afterError: afterError.values.x,
    unknown: unknown.status,
    restored: restored.values.x,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
