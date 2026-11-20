import { array, constantFrom } from "fast-check";
import { init } from "z3-solver";
import {
  type CounterAction,
  counterModel,
  createCounter,
} from "../examples/durable-counter.js";
import type { TlaSourceInput } from "../packages/idealis/src/contracts.js";
import {
  checkConformance,
  checkModel,
  checkTrace,
} from "../packages/idealis/src/index.js";
import {
  checkSafety,
  compileTlaSource,
  createPortableWasm,
} from "../packages/idealis/src/portable.js";
import { checkProperty } from "../packages/idealis/src/property.js";
import { createSmtOracle } from "../packages/idealis/src/smt.js";
import { checkTlaSource } from "../packages/idealis/src/tla-source.js";
import {
  compileTlaSafety,
  generateTlaOracle,
} from "../packages/idealis/src/tla.js";

export const runGenerated = async (
  inputs: readonly TlaSourceInput[],
  assetBase = "",
) => {
  const core = await createPortableWasm(
    new Uint8Array(await (await fetch(`${assetBase}/core.wasm`)).arrayBuffer()),
  );
  return inputs.map((input) => {
    const bounds = {
      maxDepth: 64,
      maxStates: 10000,
      maxWork: 1000000,
    };
    const program = compileTlaSource(core, input);
    const result = checkSafety(core, program, bounds);
    if (
      JSON.stringify(checkTlaSource(core, input, bounds)) !==
      JSON.stringify(result)
    )
      throw new Error("Browser generated source and compiled IR differ");
    return { version: program.version, ...result };
  });
};

export const run = async (
  assetBase = "",
  z3Api?: Awaited<ReturnType<typeof init>>,
): Promise<Record<string, unknown>> => {
  const model = checkModel(counterModel, { maxDepth: 2, maxStates: 100 });
  const trace = model.traces.at(-1);
  if (!trace) throw new Error("No browser oracle trace");
  const replay = await checkTrace(trace, createCounter());
  const property = await checkProperty(
    counterModel,
    () => createCounter({ persist: false }),
    array(
      constantFrom<CounterAction>(
        { type: "add", id: "a", amount: 1 },
        { type: "crash" },
        { type: "restart" },
      ),
      { maxLength: 20 },
    ),
    { seed: 42, numRuns: 100 },
  );
  const tla = await generateTlaOracle(
    {
      source:
        "---- MODULE Browser ----\nEXTENDS Naturals\nVARIABLE value\nInit == value = 0\nNext == value' = value + 1\nSafe == value < 2\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    {
      name: "browser-tla",
      observation: (step) => step.state.value,
      action: () => "increment",
    },
  );
  const api =
    z3Api ?? (await init({ locateFile: (path) => `${assetBase}/${path}` }));
  const rust = await createPortableWasm(
    new Uint8Array(await (await fetch(`${assetBase}/core.wasm`)).arrayBuffer()),
  );
  const safety = checkSafety(
    rust,
    compileTlaSafety({
      source:
        "---- MODULE BrowserSafety ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nSafe == x < 3\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
    }),
    { maxDepth: 4, maxStates: 5, maxWork: 100 },
  );
  if (
    safety.status !== "invariant-violation" ||
    safety.violation?.trace.length !== 4
  )
    throw new Error("Browser TLA compiler and Rust backend differ");
  const live = checkSafety(
    rust,
    compileTlaSafety({
      source:
        "---- MODULE BrowserLive ----\nEXTENDS Naturals, Sequences\nVARIABLE q\nInit == q = <<0>>\nNext == q' = <<1 - Head(q)>>\nSpec == Init /\\ [][Next]_q /\\ WF_q(Next)\nP == <>[](Head(q) = 1)\n====",
      config: "SPECIFICATION Spec\nPROPERTY P",
    }),
    { maxDepth: 4, maxStates: 10, maxWork: 10000, maxTemporalNodes: 100 },
  );
  if (
    live.status !== "liveness-violation" ||
    live.violation?.loopStart === undefined
  )
    throw new Error("Browser temporal compilation differs");
  const text = checkSafety(
    rust,
    compileTlaSafety({
      source:
        '---- MODULE BrowserString ----\nEXTENDS Naturals, Sequences\nVARIABLE x\nInit == x = "abc"\nNext == x\' = IF Len(x) > 1 THEN Tail(x) ELSE "abc"\nProp == Len(x) <= 3\n====',
      config: "INIT Init\nNEXT Next\nINVARIANT Prop",
    }),
    { maxDepth: 10, maxStates: 10, maxWork: 1000 },
  );
  if (text.status !== "complete" || text.states !== 3)
    throw new Error("Browser string operations differ");
  const unmatchedCase = {
    source:
      "---- MODULE BrowserCase ----\nVARIABLE x\nInit == x = CASE FALSE -> 1\nNext == UNCHANGED x\n====",
    config: "INIT Init\nNEXT Next",
  };
  for (const program of [
    compileTlaSafety(unmatchedCase),
    compileTlaSource(rust, unmatchedCase),
  ]) {
    let failed = false;
    try {
      checkSafety(rust, program, {
        maxDepth: 4,
        maxStates: 10,
        maxWork: 1000,
      });
    } catch (error) {
      if (
        !(error instanceof Error) ||
        !error.message.includes("CASE with no conditions true")
      )
        throw error;
      failed = true;
    }
    if (!failed) throw new Error("Browser unmatched CASE did not fail");
  }
  const foldInput = {
    source:
      "---- MODULE BrowserFold ----\nEXTENDS Integers, Sequences, Apalache\nVARIABLE x\nOp(a, b) == Append(a, b)\nInit == x = ApaFoldSet(Op, <<>>, {3, -2, 1})\nNext == UNCHANGED x\nSafe == x = <<-2, 1, 3>>\n====",
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  };
  for (const program of [
    compileTlaSafety(foldInput),
    compileTlaSource(rust, foldInput),
  ]) {
    const result = checkSafety(rust, program, {
      maxDepth: 2,
      maxStates: 10,
      maxWork: 1000,
    });
    if (result.status !== "complete" || result.states !== 1)
      throw new Error("Browser Apalache fold operations differ");
  }
  const smt = await createSmtOracle(api).solve(
    "(declare-const value Int) (assert (= value 7))",
    ["value"],
  );
  const conformance = await checkConformance(counterModel, createCounter(), [
    { type: "add", id: "wasm", amount: Number(smt.values.value) },
  ]);
  return {
    model: model.status,
    replay: replay.status,
    property: property.status,
    shrunk: property.counterexample?.length,
    tla: tla.check.status,
    tlaSteps: tla.trace?.steps.length,
    smt,
    conformance: conformance.status,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
