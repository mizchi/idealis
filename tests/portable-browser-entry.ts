import {
  coverageBounds,
  coverageInputs,
} from "../evals/experiments/action-coverage/cases.js";
import type { FrontendCase } from "../evals/experiments/frontend-semantics/cases.js";
import { sharing } from "../evals/experiments/p0-checker/inputs.js";
import {
  monotone,
  nodeSets,
  recordNodes,
} from "../evals/experiments/symmetry/cases.js";
import { counterModel } from "../examples/counter-model.js";
import { createCounter, isCounterAction } from "../examples/durable-counter.js";
import {
  createRtlcBackend,
  runBackend,
} from "../packages/idealis/src/backend.js";
import { checkConformance } from "../packages/idealis/src/conformance.js";
import type {
  SafetyBounds,
  SafetyProgram,
  SafetyResult,
  TlaSourceInput,
} from "../packages/idealis/src/contracts.js";
import { compileFiniteModel } from "../packages/idealis/src/finite.js";
import { createSimulationHost } from "../packages/idealis/src/host.js";
import { PortableCoreError } from "../packages/idealis/src/portable-value.js";
import {
  checkFiniteHistories,
  checkPortableConformance,
  checkPortableTrace,
  checkSafety,
  checkSafetyCoverage,
  coreDeletionCandidates,
  createHistoryRecorder,
  createPortableWasm,
  fromPortableValue,
  loadFiniteModel,
  prepareSafetyProgram,
  toPortableTrace,
  toPortableValue,
  verifyHostTape,
} from "../packages/idealis/src/portable.js";
import {
  checkTlaSource,
  compileTlaSource,
} from "../packages/idealis/src/tla-source.js";

export const run = async (assetBase = ""): Promise<Record<string, unknown>> => {
  const core = await createPortableWasm(
    new Uint8Array(await (await fetch(`${assetBase}/core.wasm`)).arrayBuffer()),
  );
  for (const input of coverageInputs) {
    const program = compileTlaSource(core, input);
    const observe = (run: () => SafetyResult) => {
      try {
        return { result: run(), error: null };
      } catch (error) {
        if (!(error instanceof PortableCoreError)) throw error;
        return {
          result: null,
          error: { code: error.code, message: error.message },
        };
      }
    };
    const normal = observe(() => checkSafety(core, program, coverageBounds));
    const measured = observe(
      () => checkSafetyCoverage(core, program, coverageBounds).result,
    );
    if (
      JSON.stringify(normal) !== JSON.stringify(measured) ||
      (input.expected === "error"
        ? normal.error === null
        : normal.result?.status !== input.expected)
    )
      throw new Error(`Browser coverage differs: ${input.name}`);
  }
  const source = {
    source:
      "---- MODULE BrowserSource ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nSafe == x < 2\n====",
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  };
  const sourceProgram = compileTlaSource(core, source);
  const sourceResult = checkTlaSource(core, source, {
    maxDepth: 10,
    maxStates: 100,
    maxWork: 10000,
  });
  for (const irVersion of [1, 2] as const) {
    const program = compileTlaSource(core, { ...source, irVersion });
    const prepared = prepareSafetyProgram(core, program);
    const bounds = { maxDepth: 10, maxStates: 100, maxWork: 10000 };
    const expected = checkSafety(core, program, bounds);
    const results = prepared.checkBatch([
      { bounds },
      { bounds: { ...bounds, maxWork: 0 } },
      { bounds },
    ]);
    if (
      JSON.stringify(prepared.check(bounds)) !== JSON.stringify(expected) ||
      results[0]?.kind !== "safety" ||
      results[1]?.kind !== "error" ||
      results[2]?.kind !== "safety" ||
      JSON.stringify(results[0].result) !== JSON.stringify(expected) ||
      JSON.stringify(results[2].result) !== JSON.stringify(expected)
    )
      throw new Error("Prepared safety or batch differs in browser");
    prepared.release();
    try {
      prepared.check(bounds);
      throw new Error("Released safety handle was accepted");
    } catch (error) {
      if (!(error instanceof PortableCoreError)) throw error;
    }
  }
  if (
    sourceProgram.name !== "BrowserSource" ||
    sourceResult.status !== "invariant-violation" ||
    sourceResult.violation?.trace.length !== 3
  )
    throw new Error("Rust source frontend differs in browser");
  const finiteInputs: {
    name: string;
    source: string;
    config: string;
    expected: string;
  }[] = await (await fetch(`${assetBase}/finite.json`)).json();
  for (const input of finiteInputs) {
    const program = compileTlaSource(core, input);
    const bounds = {
      maxDepth: 100,
      maxStates: 1000,
      maxWork: 1_000_000,
      maxTemporalNodes: 10000,
    };
    const observe = (run: () => SafetyResult): string => {
      try {
        return run().status;
      } catch (error) {
        if (!(error instanceof PortableCoreError)) throw error;
        return "model-error";
      }
    };
    if (
      observe(() => checkTlaSource(core, input, bounds)) !== input.expected ||
      observe(() => checkSafety(core, program, bounds)) !== input.expected
    )
      throw new Error(`Finite TLC support differs in browser: ${input.name}`);
  }
  const moduleInputs: (TlaSourceInput & {
    name: string;
    expected: string;
    states?: number;
    action?: string;
  })[] = [
    ...(await (await fetch(`${assetBase}/modules.json`)).json()),
    ...(await (await fetch(`${assetBase}/operators.json`)).json()),
  ];
  for (const input of moduleInputs) {
    const bounds = {
      maxDepth: 100,
      maxStates: 1000,
      maxWork: 1_000_000,
      maxTemporalNodes: 10000,
    };
    const observe = (run: () => unknown): unknown => {
      try {
        return run();
      } catch (error) {
        if (!(error instanceof PortableCoreError)) throw error;
        return null;
      }
    };
    const program = observe(() => compileTlaSource(core, input));
    if (input.expected === "unsupported") {
      if (program !== null)
        throw new Error(`Browser accepted unsupported module ${input.name}`);
      continue;
    }
    const result = observe(() => checkTlaSource(core, input, bounds));
    if (input.expected === "model-error") {
      if (program === null || result !== null)
        throw new Error(`Browser module error differs ${input.name}`);
      continue;
    }
    if (program === null || result === null)
      throw new Error(`Browser module failed ${input.name}`);
    const checked = checkTlaSource(core, input, bounds);
    if (input.action && checked.violation?.trace[1]?.action !== input.action)
      throw new Error(`Browser module action differs ${input.name}`);
    if (
      checked.status !== input.expected ||
      (input.states !== undefined && checked.states !== input.states)
    )
      throw new Error(`Browser module result differs ${input.name}`);
  }
  const actions = [
    { type: "add", id: "a", amount: 1 },
    { type: "crash" },
    { type: "restart" },
  ] as const;
  const program: SafetyProgram = await (
    await fetch(`${assetBase}/safety.json`)
  ).json();
  const safety = checkSafety(core, program, {
    maxDepth: 4,
    maxStates: 5,
    maxWork: 100,
  });
  const backendResult = await runBackend(
    createRtlcBackend(core, { version: "0.1.0-alpha.0" }),
    {
      format: "modelt.backend-request",
      version: 1,
      scope: {
        modelId: "browser-safety-v1",
        propertyId: "Safe-v1",
        semantics: "tlc-1.7.4-int32",
        assumptions: [],
      },
      query: {
        kind: "explicit",
        bounds: { maxDepth: 4, maxStates: 5, maxWork: 100 },
      },
      input: program,
    },
  );
  if (
    backendResult.outcome.kind !== "counterexample" ||
    backendResult.outcome.witness.kind !== "behavior"
  )
    throw new Error("Browser backend contract differs");
  if (
    safety.status !== "invariant-violation" ||
    safety.violation?.trace.length !== 4
  )
    throw new Error("Browser safety backend differs");
  const complex: {
    program: SafetyProgram;
    bounds?: SafetyBounds;
    expected: SafetyResult["status"];
  }[] = await (await fetch(`${assetBase}/complex.json`)).json();
  for (const input of complex) {
    const result = checkSafety(
      core,
      input.program,
      input.bounds ?? {
        maxDepth: 100,
        maxStates: 10000,
        maxWork: 2000000,
        maxTemporalNodes: 50000,
      },
    );
    if (result.status !== input.expected)
      throw new Error(`Browser complex backend: ${JSON.stringify(result)}`);
  }
  const reference = await checkConformance(
    counterModel,
    createCounter(),
    actions,
  );
  const normal = await checkPortableConformance(
    core,
    counterModel,
    createCounter(),
    actions,
  );
  const broken = await checkPortableConformance(
    core,
    counterModel,
    createCounter({ persist: false }),
    actions,
  );
  const replay = await checkPortableTrace(
    core,
    toPortableTrace(reference.trace),
    createCounter({ persist: false }),
    (value) => {
      const action = fromPortableValue(value);
      if (!isCounterAction(action)) throw new Error("Invalid portable action");
      return action;
    },
  );
  const value = toPortableValue({
    integer: 9007199254740993123456789n,
    set: new Set([2, 1, 2]),
    map: new Map([["😀", new Uint8Array([255])]]),
  });
  const result = core.request({
    version: 1,
    request: { kind: "canonicalize", value },
  });
  if (
    result.kind !== "value" ||
    JSON.stringify(result.value) !== JSON.stringify(value)
  )
    throw new Error("Browser value semantics differ");
  const finite = compileFiniteModel(counterModel, {
    maxDepth: 20,
    maxStates: 1000,
  });
  if (
    !finite.model ||
    checkFiniteHistories(core, finite.model, [
      toPortableTrace(reference.trace),
    ])[0]?.status !== "matched"
  )
    throw new Error("Browser finite artifact differs");
  if (!coreDeletionCandidates(core, 3).some((range) => range.count === 1))
    throw new Error("Missing browser shrink candidates");
  const loaded = loadFiniteModel(core, finite.model);
  const history = createHistoryRecorder(
    counterModel.name,
    toPortableValue(reference.trace.initial),
  );
  for (const [index, step] of reference.trace.steps.entries()) {
    history.invoke(String(index), toPortableValue(step.action));
    history.returned(String(index), toPortableValue(step.observation));
  }
  if (
    loaded.checkHistory(history.snapshot(), 100).status !== "linearizable" ||
    loaded.checkHistories([toPortableTrace(reference.trace)])[0]?.status !==
      "matched"
  )
    throw new Error("Browser loaded history differs");
  loaded.release();
  const host = createSimulationHost({ seed: 3 });
  host.write("value", 1);
  host.persist();
  host.write("value", 2);
  host.crash();
  host.restart();
  if (host.read("value") !== 1)
    throw new Error("Browser storage recovery differs");
  if (
    JSON.stringify(verifyHostTape(core, host.trace())) !==
    JSON.stringify(
      toPortableValue({
        clock: 0,
        running: true,
        storage: { value: 1 },
        pending: [],
      }),
    )
  )
    throw new Error("Rust browser host replay differs");
  return {
    source: sourceResult.status,
    backend: backendResult.outcome.kind,
    normal: normal.status,
    broken: broken.status,
    replay: replay.status,
    steps: broken.checkedSteps,
    isolated: globalThis.crossOriginIsolated ?? false,
    coverageCases: coverageInputs.length,
  };
};

export const runSymmetry = async (assetBase = "") => {
  const core = await createPortableWasm(
    new Uint8Array(await (await fetch(`${assetBase}/core.wasm`)).arrayBuffer()),
  );
  const input = {
    source:
      "---- MODULE BrowserSymmetry ----\nEXTENDS Naturals,TLC\nCONSTANT Nodes\nVARIABLE f\nSymm == Permutations(Nodes)\nInit == f = [n \\in Nodes |-> 0]\nNext == \\E n \\in Nodes: f' = [f EXCEPT ![n] = IF @ < 2 THEN @+1 ELSE 0]\nSafe == f \\in [Nodes -> 0..2]\n====",
    config:
      "CONSTANT Nodes={a,b,c}\nINIT Init\nNEXT Next\nINVARIANT Safe\nSYMMETRY Symm",
  };
  const bounds = { maxDepth: 100, maxStates: 1000, maxWork: 1000000 };
  const reduced = checkSafety(core, compileTlaSource(core, input), bounds);
  const records = checkSafety(
    core,
    compileTlaSource(core, recordNodes(3)),
    bounds,
  );
  const sets = checkSafety(core, compileTlaSource(core, nodeSets(3)), bounds);
  const por = checkSafety(core, compileTlaSource(core, monotone(3)), bounds, {
    partialOrder: true,
  });
  const actions = checkSafety(core, compileTlaSource(core, input), bounds, {
    symmetryActions: true,
  });
  const original = checkSafety(
    core,
    compileTlaSource(core, {
      ...input,
      config: input.config.replace("\nSYMMETRY Symm", ""),
    }),
    bounds,
  );
  return {
    status: reduced.status,
    states: reduced.states,
    generated: reduced.generated,
    records: {
      status: records.status,
      states: records.states,
      generated: records.generated,
    },
    sets: {
      status: sets.status,
      states: sets.states,
      generated: sets.generated,
    },
    actions: {
      status: actions.status,
      states: actions.states,
      generated: actions.generated,
    },
    originalStates: original.states,
    por: { status: por.status, states: por.states, generated: por.generated },
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};

export const runSharedIr = async (assetBase = "") => {
  const core = await createPortableWasm(
    new Uint8Array(await (await fetch(`${assetBase}/core.wasm`)).arrayBuffer()),
  );
  const bounds = { maxDepth: 64, maxStates: 10000, maxWork: 1000000 };
  const program = compileTlaSource(core, { ...sharing(60), irVersion: 2 });
  const result = checkSafety(core, program, bounds);
  const bounded = checkTlaSource(
    core,
    { ...sharing(60, true), irVersion: 2 },
    { ...bounds, maxWork: 256 },
  );
  return {
    version: program.version,
    status: result.status,
    states: result.states,
    generated: result.generated,
    bounded: {
      status: bounded.status,
      reason: bounded.reason,
      work: bounded.work,
    },
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};

export const runFrontendCompletion = async (assetBase = "") => {
  const core = await createPortableWasm(
    new Uint8Array(await (await fetch(`${assetBase}/core.wasm`)).arrayBuffer()),
  );
  const inputs: (TlaSourceInput & {
    name: string;
    versions: readonly (1 | 2)[];
    status: string;
    states: number | null;
  })[] = await (await fetch(`${assetBase}/completion.json`)).json();
  const bounds = {
    maxDepth: 100,
    maxStates: 1000,
    maxWork: 1000000,
    maxTemporalNodes: 10000,
  };
  let matched = 0;
  let rejected = 0;
  for (const input of inputs)
    for (const irVersion of input.versions) {
      const source = { ...input, irVersion };
      if (input.status === "error") {
        try {
          compileTlaSource(core, source);
        } catch (error) {
          if (!(error instanceof PortableCoreError)) throw error;
          rejected++;
          continue;
        }
        throw new Error(
          `Browser accepted invalid TLA declaration ${input.name}`,
        );
      }
      const program = compileTlaSource(core, source);
      const checked = checkTlaSource(core, source, bounds);
      if (
        checked.status !== input.status ||
        checked.states !== input.states ||
        JSON.stringify(checked) !==
          JSON.stringify(checkSafety(core, program, bounds))
      )
        throw new Error(`Browser frontend differs: ${input.name}/${irVersion}`);
      matched++;
    }
  const loop = {
    source:
      "---- MODULE Test ----\nVARIABLE x\nRECURSIVE Loop(_)\nLoop(value) == Loop(value)\nInit == x=Loop(0)\nNext == UNCHANGED x\n====",
    config: "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE",
    irVersion: 2 as const,
    syntaxVersion: "1.8.0" as const,
  };
  const nontermination = checkTlaSource(core, loop, bounds);
  if (
    nontermination.status !== "bounded" ||
    nontermination.reason !== "evaluation"
  )
    throw new Error("Recursive evaluation exhaustion became a browser success");
  const unicode = checkTlaSource(
    core,
    {
      source:
        "---- MODULE Test ----\nVARIABLE x\nInit ≜ x=1\nNext ≜ UNCHANGED x\nSafe ≜ ∀ value ∈ {1}: value=x\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
      syntaxVersion: "1.8.0",
    },
    bounds,
  );
  if (unicode.status !== "complete")
    throw new Error("Modern Unicode source failed in browser");
  return {
    matched,
    rejected,
    evaluation: nontermination.reason,
    unicode: unicode.status,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};

const hopSource = (levels: number): string =>
  [
    "---- MODULE CallDepth ----",
    "EXTENDS Integers",
    "VARIABLE counter",
    "hop0(x) == x + 1",
    ...Array.from(
      { length: levels },
      (_, index) => `hop${index + 1}(x) == hop${index}(x) + 1`,
    ),
    "init == counter = 0",
    "inv == counter >= 0",
    `step == counter' = hop${levels}(counter) % 5`,
    "q_init == init",
    "q_step == step",
    "q_inv == inv",
    "====",
    "",
  ].join("\n");

export const runCallDepth = async (assetBase = "") => {
  const core = await createPortableWasm(
    new Uint8Array(await (await fetch(`${assetBase}/core.wasm`)).arrayBuffer()),
  );
  const bounds = { maxDepth: 64, maxStates: 10000, maxWork: 1_000_000 };
  const config =
    "INIT q_init\nNEXT q_step\nINVARIANT q_inv\nCHECK_DEADLOCK FALSE\n";
  const check = (levels: number) =>
    checkTlaSource(
      core,
      { source: hopSource(levels), config, irVersion: 2 },
      bounds,
    );
  const summary = (result: SafetyResult) => ({
    status: result.status,
    states: result.states,
    generated: result.generated,
    work: result.work,
  });
  const accepted = summary(check(83));
  let refusal: { code: string; message: string } | null = null;
  try {
    check(84);
  } catch (error) {
    if (!(error instanceof PortableCoreError)) throw error;
    refusal = { code: error.code, message: error.message };
  }
  if (refusal === null)
    throw new Error("Browser accepted layered calls past the limit");
  const afterRefusal = summary(check(40));
  return {
    accepted,
    refusal,
    afterRefusal,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};

export const runRecursiveActions = async (assetBase = "") => {
  const core = await createPortableWasm(
    new Uint8Array(await (await fetch(`${assetBase}/core.wasm`)).arrayBuffer()),
  );
  const inputs: (TlaSourceInput & {
    name: string;
    status: string;
    states: number;
    generated: number;
  })[] = await (await fetch(`${assetBase}/recursive-actions.json`)).json();
  const bounds = {
    maxDepth: 1000,
    maxStates: 2000,
    maxWork: 1_000_000,
    maxTemporalNodes: 10000,
  };
  let matched = 0;
  let rejected = 0;
  let bounded = 0;
  for (const input of inputs) {
    if (input.status === "error") {
      try {
        compileTlaSource(core, input);
      } catch (error) {
        if (!(error instanceof PortableCoreError)) throw error;
        rejected++;
        continue;
      }
      throw new Error(
        `Browser accepted invalid recursive action ${input.name}`,
      );
    }
    const program = compileTlaSource(core, input);
    const checked = checkTlaSource(core, input, bounds);
    if (
      checked.status !== input.status ||
      JSON.stringify(checked) !==
        JSON.stringify(checkSafety(core, program, bounds))
    )
      throw new Error(`Browser recursive action differs: ${input.name}`);
    if (input.status === "bounded") {
      if (checked.reason !== "evaluation" || checked.violation !== null)
        throw new Error(`Browser recursion cut became success: ${input.name}`);
      bounded++;
    } else {
      if (
        checked.states !== input.states ||
        checked.generated !== input.generated
      )
        throw new Error(`Browser recursive counts differ: ${input.name}`);
      matched++;
    }
    const partial = checkSafety(core, program, { ...bounds, maxWork: 1 });
    if (
      partial.status !== "bounded" ||
      partial.reason !== "work" ||
      partial.work !== 1
    )
      throw new Error(`Browser recursive work cut differs: ${input.name}`);
  }
  return {
    matched,
    rejected,
    bounded,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};

export const runFrontendSemantics = async (assetBase = "") => {
  const bytes = new Uint8Array(
    await (await fetch(`${assetBase}/core.wasm`)).arrayBuffer(),
  );
  const module = await WebAssembly.compile(bytes);
  if (WebAssembly.Module.imports(module).length !== 0)
    throw new Error("Frontend semantics requires host imports");
  const core = await createPortableWasm(bytes);
  const inputs: readonly FrontendCase[] = await (
    await fetch(`${assetBase}/frontend-semantics.json`)
  ).json();
  const bounds = {
    maxDepth: 1000,
    maxStates: 2000,
    maxWork: 1_000_000,
    maxTemporalNodes: 10000,
  };
  let matched = 0;
  let rejected = 0;
  let modelErrors = 0;
  let bounded = 0;
  for (const input of inputs) {
    if (input.expected === "input-error") {
      try {
        compileTlaSource(core, input);
      } catch (error) {
        if (!(error instanceof PortableCoreError)) throw error;
        rejected++;
        continue;
      }
      throw new Error(`Browser accepted invalid level: ${input.name}`);
    }
    const program = compileTlaSource(core, input);
    if (input.expected === "evaluation-error") {
      try {
        checkSafety(core, program, bounds);
      } catch (error) {
        if (!(error instanceof PortableCoreError)) throw error;
        modelErrors++;
        continue;
      }
      throw new Error(`Browser suppressed model error: ${input.name}`);
    }
    const checked = checkSafety(core, program, bounds);
    const expected =
      input.expected === "temporal-violation"
        ? "liveness-violation"
        : input.expected;
    if (checked.status !== expected)
      throw new Error(`Browser frontend semantics differs: ${input.name}`);
    if (input.expected === "bounded") {
      if (checked.reason !== "evaluation" || checked.violation !== null)
        throw new Error(`Browser recursion became success: ${input.name}`);
      bounded++;
    } else matched++;
    const partial = checkSafety(core, program, { ...bounds, maxWork: 1 });
    if (
      partial.status !== "bounded" ||
      partial.reason !== "work" ||
      partial.work !== 1
    )
      throw new Error(`Browser work budget differs: ${input.name}`);
  }
  return {
    matched,
    rejected,
    modelErrors,
    bounded,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
