import type {
  TestingCase,
  TestingVerdict,
} from "../../../packages/idealis/src/testing.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";

export interface Evaluation {
  readonly verdict: TestingVerdict;
  readonly observations: readonly number[];
  readonly actual: number;
  readonly expected: number;
}
export const evaluate = async (
  actors: number,
  testCase: TestingCase,
  broken: boolean,
): Promise<Evaluation> => {
  if (
    !Number.isInteger(actors) ||
    actors < 1 ||
    actors > 8 ||
    testCase.actions.length > 128
  )
    return {
      verdict: { status: "invalid" },
      observations: [],
      actual: 0,
      expected: 0,
    };
  const positions = Array.from({ length: actors }, () => 0);
  const order: { actor: number; operation: string }[] = [];
  for (const action of testCase.actions) {
    const actor =
      isRecord(action) && typeof action.actor === "string"
        ? Number(action.actor.slice(6))
        : -1;
    if (
      !isRecord(action) ||
      Object.keys(action).length !== 3 ||
      !Number.isInteger(actor) ||
      actor < 0 ||
      actor >= actors ||
      action.actor !== `actor-${actor}` ||
      action.step !== positions[actor] ||
      (positions[actor] ?? 0) > 1 ||
      action.operation !== ["read", "write"][positions[actor] ?? -1]
    )
      return {
        verdict: { status: "invalid" },
        observations: [],
        actual: 0,
        expected: 0,
      };
    positions[actor] = (positions[actor] ?? 0) + 1;
    order.push({ actor, operation: String(action.operation) });
  }
  const counter = { value: 0 };
  const increment = async function* () {
    const snapshot = counter.value;
    yield "read";
    counter.value = broken ? snapshot + 1 : counter.value + 1;
    yield "write";
  };
  const workers = Array.from({ length: actors }, () => increment());
  const observations: number[] = [];
  let expected = 0;
  let mismatch = false;
  try {
    for (const action of order) {
      const next = await workers[action.actor]?.next();
      if (!next || next.done || next.value !== action.operation)
        throw new Error("Unexpected actor operation");
      expected += Number(action.operation === "write");
      mismatch ||= counter.value !== expected;
      observations.push(counter.value);
    }
  } finally {
    for (const worker of workers) await worker.return();
  }
  return {
    verdict: mismatch
      ? { status: "mismatch", signature: "lost-update" }
      : { status: "matched" },
    observations,
    actual: counter.value,
    expected,
  };
};
