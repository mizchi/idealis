import {
  type CounterAction,
  counterModel,
  createCounter,
  isCounterAction,
} from "../../../examples/durable-counter.js";
import { checkConformance } from "../../../packages/idealis/src/conformance.js";
import type {
  TestingCase,
  TestingVerdict,
} from "../../../packages/idealis/src/testing.js";
export const actions = (testCase: TestingCase): readonly CounterAction[] =>
  testCase.actions.map((action) => {
    if (!isCounterAction(action))
      throw new Error("Invalid counter action in generated case");
    return action;
  });
export const evaluate = async (
  testCase: TestingCase,
  broken: boolean,
): Promise<TestingVerdict> => {
  const result = await checkConformance(
    counterModel,
    createCounter({ persist: !broken }),
    actions(testCase),
  );
  return result.status === "mismatch"
    ? { status: "mismatch", signature: "lost-durable-write" }
    : { status: result.status };
};
