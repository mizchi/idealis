import assert from "node:assert/strict";
import {
  counterModel,
  createCounter,
} from "../../../examples/durable-counter.js";
import {
  checkConformance,
  checkDifferential,
} from "../../../packages/idealis/src/conformance.js";
import type {
  TestingCase,
  TestingJson,
} from "../../../packages/idealis/src/testing.js";
import { actions } from "./driver.js";

export { evaluate } from "./driver.js";
export { plan } from "./fixture.js";

export const differentialCase = async (testCase: TestingCase) => {
  const record = async (broken: boolean): Promise<readonly TestingJson[]> => {
    const driver = createCounter({ persist: !broken });
    try {
      const observations: TestingJson[] = [{ ...(await driver.reset()) }];
      for (const action of actions(testCase))
        observations.push({ ...(await driver.step(action)) });
      return observations;
    } finally {
      await driver.dispose?.();
    }
  };
  const reference = await record(false);
  const candidate = await record(true);
  const expected = await checkDifferential(
    createCounter(),
    createCounter({ persist: false }),
    actions(testCase),
  );
  return {
    oracle: { kind: "allowed" as const, values: [reference] },
    targets: [
      { id: "normal", outcome: { kind: "value" as const, value: reference } },
      { id: "volatile", outcome: { kind: "value" as const, value: candidate } },
    ],
    expected: expected.status,
  };
};
export const reproduce = async (testCase: TestingCase) => {
  const broken = await checkConformance(
    counterModel,
    createCounter({ persist: false }),
    actions(testCase),
  );
  const fixed = await checkConformance(
    counterModel,
    createCounter(),
    actions(testCase),
  );
  assert.equal(broken.status, "mismatch");
  assert.equal(fixed.status, "matched");
  return { broken, fixed };
};
