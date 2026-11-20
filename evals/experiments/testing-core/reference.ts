import assert from "node:assert/strict";
import { shrinkCounterexample } from "../../../packages/idealis/src/shrink.js";
import type {
  TestingCase,
  TestingGenerator,
  TestingJson,
  TestingPlan,
  TestingVerdict,
} from "../../../packages/idealis/src/testing.js";
import { equalValues } from "../../../packages/idealis/src/trace.js";

export const accepts = (
  generator: TestingGenerator,
  actions: readonly TestingJson[],
): boolean => {
  if (
    actions.length < generator.minLength ||
    actions.length > generator.maxLength
  )
    return false;
  let states = new Set(generator.initial);
  for (const action of actions)
    states = new Set(
      [...states].flatMap((state) =>
        (generator.states[state] ?? [])
          .filter((edge) => equalValues(edge.action, action))
          .map((edge) => edge.target),
      ),
    );
  return states.size > 0;
};
export const generate = (generator: TestingGenerator, seed: number) => {
  let random = seed;
  let index = 0;
  const choose = (length: number): number => {
    random ^= random << 13;
    random ^= random >>> 17;
    random ^= random << 5;
    return (random >>> 0) % length;
  };
  return (): TestingCase | null => {
    const length =
      generator.minLength +
      choose(generator.maxLength - generator.minLength + 1);
    let state = generator.initial[choose(generator.initial.length)];
    const actions: TestingJson[] = [];
    for (let step = 0; step < length; step++) {
      const edges = generator.states[state ?? ""] ?? [];
      if (!edges.length) break;
      const edge = edges[choose(edges.length)];
      assert.ok(edge);
      actions.push(edge.action);
      state = edge.target;
    }
    const current = index++;
    return actions.length < generator.minLength
      ? null
      : {
          format: "idealis.test-case",
          version: 1,
          rng: "xorshift32-v1",
          generatorId: generator.id,
          generatorVersion: generator.version,
          seed,
          index: current,
          actions,
        };
  };
};
export const property = async (
  plan: TestingPlan,
  evaluate: (testCase: TestingCase) => Promise<TestingVerdict>,
) => {
  const next = generate(plan.generator, plan.seed);
  const generated: TestingCase[] = [];
  let cases = 0;
  for (let attempt = 0; attempt < plan.maxAttempts; attempt++) {
    const testCase = next();
    if (!testCase) continue;
    generated.push(testCase);
    const verdict = await evaluate(testCase);
    if (verdict.status === "matched") {
      if (++cases === plan.cases)
        return {
          status: "passed",
          generated,
          cases,
          counterexample: null,
          shrink: null,
        };
    } else if (verdict.status === "mismatch") {
      const shrunk = await shrinkCounterexample(
        testCase.actions,
        async (candidate) => {
          const result = await evaluate({ ...testCase, actions: candidate });
          if (result.status === "unknown" || result.status === "error")
            throw new Error(
              "Reference shrink corpus has an inconclusive evaluation",
            );
          return result;
        },
        {
          maxChecks: plan.maxShrinkChecks,
          isEnabled: (actions) => accepts(plan.generator, actions),
        },
      );
      return {
        status: "failed",
        generated,
        cases,
        counterexample: { ...testCase, actions: shrunk.actions },
        shrink: { status: shrunk.status, checks: shrunk.checks },
      };
    } else if (verdict.status !== "invalid")
      throw new Error("Reference corpus must produce a definite verdict");
  }
  return {
    status: "exhausted",
    generated,
    cases,
    counterexample: null,
    shrink: null,
  };
};
