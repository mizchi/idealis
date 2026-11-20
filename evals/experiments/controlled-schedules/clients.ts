import assert from "node:assert/strict";
import type { TestingCase } from "../../../packages/idealis/src/testing.js";
import { processClient } from "../smt-subset/clients.js";
import { evaluate } from "./driver.js";

export const sutPath =
  "evals/experiments/controlled-schedules/rust/target/release/idealis-schedule-eval";
export const createSut = () => {
  const client = processClient(sutPath, [], true);
  const exchanges: { input: unknown; response: unknown }[] = [];
  return {
    exchanges,
    close: client.close,
    evaluate: async (
      actors: number,
      testCase: TestingCase,
      broken: boolean,
    ) => {
      const input = { actors, actions: testCase.actions, broken };
      const response: unknown = JSON.parse(
        await client.request(JSON.stringify(input)),
      );
      exchanges.push({ input, response });
      const expected = await evaluate(actors, testCase, broken);
      assert.deepEqual(response, expected);
      return expected;
    },
  };
};
