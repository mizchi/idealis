import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  createTestingWasm,
  type TestingResult,
} from "../../../packages/idealis/src/testing.js";
import { assertOracle } from "../workspace-oracle/node.js";
import { createNative, wasmPath } from "./clients.js";
import { property } from "./reference.js";
import { differentialCase, evaluate, plan, reproduce } from "./scenario.js";

export const verify = async (
  seeds: readonly number[],
  onRecord: (record: unknown) => void,
) => {
  const native = createNative();
  const wasm = await createTestingWasm(await readFile(wasmPath));
  let detected = 0;
  try {
    for (const seed of seeds) {
      const input = plan(seed);
      const variants: {
        broken: boolean;
        result: TestingResult;
        reproduction: unknown;
      }[] = [];
      for (const broken of [false, true]) {
        const result = await native.runtime.property(input, (testCase) =>
          evaluate(testCase, broken),
        );
        const wasmResult = await wasm.property(input, (testCase) =>
          evaluate(testCase, broken),
        );
        assertOracle(wasmResult, result);
        const reference = await property(input, (testCase) =>
          evaluate(testCase, broken),
        );
        assert.equal(result.status, reference.status);
        assert.equal(result.cases, reference.cases);
        assertOracle(
          result.trials
            .filter((trial) => trial.phase === "generation")
            .map((trial) => trial.case),
          reference.generated,
        );
        assertOracle(result.counterexample, reference.counterexample);
        assertOracle(result.shrink, reference.shrink);
        if (!broken) assert.equal(result.status, "passed");
        let reproduction: unknown = null;
        if (result.counterexample) {
          reproduction = await reproduce(result.counterexample);
          detected++;
          assert.equal(
            (
              await native.runtime.replay(
                result.counterexample,
                (testCase) => evaluate(testCase, true),
                result.signature ?? undefined,
              )
            ).status,
            "failed",
          );
          assert.equal(
            (
              await wasm.replay(result.counterexample, (testCase) =>
                evaluate(testCase, false),
              )
            ).status,
            "passed",
          );
        }
        const first = result.counterexample ?? result.trials[0]?.case;
        assert.ok(first);
        const comparison = await differentialCase(first);
        const compared = await native.runtime.compare(
          comparison.oracle,
          comparison.targets,
        );
        assertOracle(
          await wasm.compare(comparison.oracle, comparison.targets),
          compared,
        );
        assert.equal(compared.status, comparison.expected);
        assert.equal(compared.comparisons[0]?.status, "matched");
        variants.push({ broken, result, reproduction });
      }
      onRecord({ seed, variants });
    }
    return { seeds: seeds.length, detected, exchanges: native.exchanges };
  } finally {
    await native.close();
  }
};
