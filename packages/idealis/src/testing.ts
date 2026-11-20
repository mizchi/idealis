import type {
  TestingCase,
  TestingEvaluator,
  TestingGenerator,
  TestingObservation,
  TestingPlan,
  TestingRequest,
  TestingResult,
  TestingRuntime,
  TestingSchedule,
  TestingTarget,
} from "./contracts.js";
import {
  differential,
  effect,
  generator,
  json,
  reply,
  TestingError,
  verdict,
} from "./testing-wire.js";

export type {
  TestingCase,
  TestingComparison,
  TestingComparisonStatus,
  TestingDifferential,
  TestingEffect,
  TestingEvaluator,
  TestingGenerator,
  TestingJson,
  TestingObservation,
  TestingPhase,
  TestingPlan,
  TestingRequest,
  TestingResult,
  TestingRuntime,
  TestingSchedule,
  TestingScheduleActor,
  TestingTarget,
  TestingTrial,
  TestingVerdict,
} from "./contracts.js";
export { TestingError } from "./testing-wire.js";

export const createTestingRuntime = (
  dispatch: (request: TestingRequest) => unknown | Promise<unknown>,
): TestingRuntime => {
  let busy = false;
  const run = async (
    request: TestingRequest,
    evaluate: TestingEvaluator,
  ): Promise<TestingResult> => {
    if (busy)
      throw new TestingError(
        "protocol",
        "Testing runtime already has an active session",
      );
    busy = true;
    let started = false;
    try {
      const snapshot: TestingRequest = JSON.parse(
        JSON.stringify(json(request)),
      );
      started = true;
      let current = effect(reply(await dispatch(snapshot), "effect").effect);
      while (current.kind === "evaluate") {
        const response = verdict(await evaluate(current.case, current.phase));
        current = effect(
          reply(
            await dispatch({
              kind: "resume",
              id: current.id,
              verdict: response,
            }),
            "effect",
          ).effect,
        );
      }
      return current.result;
    } finally {
      try {
        if (started) reply(await dispatch({ kind: "cancel" }), "cancelled");
      } finally {
        busy = false;
      }
    }
  };
  return Object.freeze({
    schedule: async (schedule: TestingSchedule) => {
      const request: TestingRequest = JSON.parse(
        JSON.stringify(json({ kind: "schedule", schedule })),
      );
      return generator(reply(await dispatch(request), "generator").generator);
    },
    property: (plan: TestingPlan, evaluate: TestingEvaluator) =>
      run({ kind: "property", plan }, evaluate),
    shrink: (
      generator: TestingGenerator,
      testCase: TestingCase,
      evaluate: TestingEvaluator,
      budget: { readonly maxChecks: number; readonly maxWork: number },
    ) =>
      run(
        {
          kind: "shrink",
          generator,
          case: testCase,
          maxChecks: budget.maxChecks,
          maxWork: budget.maxWork,
        },
        evaluate,
      ),
    replay: (
      testCase: TestingCase,
      evaluate: TestingEvaluator,
      expectedSignature?: string,
    ) =>
      run(
        {
          kind: "replay",
          case: testCase,
          expectedSignature: expectedSignature ?? null,
        },
        evaluate,
      ),
    compare: async (
      oracle: TestingObservation,
      targets: readonly TestingTarget[],
    ) => {
      const request = { kind: "compare" as const, oracle, targets };
      json(request);
      return differential(reply(await dispatch(request), "comparison").result);
    },
  });
};

export const createTestingWasm = async (
  bytes: Uint8Array | WebAssembly.Module,
): Promise<TestingRuntime> => {
  const module =
    bytes instanceof WebAssembly.Module
      ? bytes
      : await WebAssembly.compile(new Uint8Array(bytes).buffer);
  if (WebAssembly.Module.imports(module).length)
    throw new TestingError(
      "protocol",
      "Testing Wasm must not import host functions",
    );
  const wasm = await WebAssembly.instantiate(module);
  const memory = wasm.exports.memory;
  if (!(memory instanceof WebAssembly.Memory))
    throw new TestingError("protocol", "Missing testing memory");
  const method = (name: string) => {
    const value = wasm.exports[`testing_${name}`];
    if (typeof value !== "function")
      throw new TestingError("protocol", `Missing testing ${name}`);
    return value;
  };
  if (method("abi_version")() !== 1)
    throw new TestingError("protocol", "Unsupported testing ABI");
  const resize = method("input_resize");
  const dispatch = method("dispatch");
  const pointer = method("output_ptr");
  const length = method("output_len");
  const integer = (input: unknown): number => {
    if (typeof input !== "number" || !Number.isInteger(input))
      throw new TestingError("protocol", "Expected testing ABI integer");
    return input >>> 0;
  };
  return createTestingRuntime((request) => {
    const input = new TextEncoder().encode(
      JSON.stringify(json({ version: 1, request })),
    );
    if (input.length > 1_048_576)
      throw new TestingError("resource", "Testing request exceeds 1 MiB");
    const start = integer(resize(input.length));
    if (start + input.length > memory.buffer.byteLength)
      throw new TestingError("protocol", "Invalid testing input buffer");
    new Uint8Array(memory.buffer, start, input.length).set(input);
    dispatch();
    const output = integer(pointer());
    const size = integer(length());
    if (output + size > memory.buffer.byteLength)
      throw new TestingError("protocol", "Invalid testing output buffer");
    return JSON.parse(
      new TextDecoder().decode(new Uint8Array(memory.buffer, output, size)),
    );
  });
};
