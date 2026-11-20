import {
  actions,
  evaluate,
} from "../../evals/experiments/testing-core/driver.js";
import { plan } from "../../evals/experiments/testing-core/fixture.js";
import { createTestingWasm } from "../../packages/idealis/src/testing.js";

export const run = async (assetBase = "") => {
  const runtime = await createTestingWasm(
    new Uint8Array(
      await (await fetch(`${assetBase}/testing.wasm`)).arrayBuffer(),
    ),
  );
  const input = plan(0x36a1);
  const normal = await runtime.property(input, (testCase) =>
    evaluate(testCase, false),
  );
  const broken = await runtime.property(input, (testCase) =>
    evaluate(testCase, true),
  );
  const testCase = broken.counterexample;
  if (!testCase) throw new Error("Missing counterexample");
  return {
    normal: normal.status,
    broken: broken.status,
    actions: actions(testCase).map((action) => action.type),
    shrink: broken.shrink?.status,
    replay: (
      await runtime.replay(
        testCase,
        (testCase) => evaluate(testCase, true),
        broken.signature ?? undefined,
      )
    ).status,
    fixed: (
      await runtime.replay(testCase, (testCase) => evaluate(testCase, false))
    ).status,
    unknown: (
      await runtime.property({ ...input, maxWork: 0 }, (testCase) =>
        evaluate(testCase, false),
      )
    ).status,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
