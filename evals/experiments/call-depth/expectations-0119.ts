import assert from "node:assert/strict";
import type { Baseline, ChainCase } from "../nesting-chains/cases.js";
import {
  type Cell,
  type Expectation,
  type Frontend,
  type Outcome,
} from "../nesting-chains/outcome.js";
import { sources0075 } from "../nesting-chains/sources-0075.js";
import { type DepthSource, depthLimit, sources0119 } from "./sources.js";

export const formerLimit = "Safety evaluation nesting exceeds 128";
export const currentLimit = `Safety evaluation nesting exceeds ${depthLimit}`;
export const overriddenNames = ["calls-127", "calls-128", "calls-129"] as const;

export interface Records0119 {
  readonly before: Baseline;
  readonly oracle: Baseline;
}

export interface DepthCase extends ChainCase {
  readonly depth: number | null;
}

const recorded = (baseline: Baseline, name: string): Expectation => {
  const found = baseline[name];
  assert.ok(found, `baseline for ${name}`);
  return found;
};

export const formerRefusal = (outcome: Outcome): boolean =>
  outcome.stage === "check" && outcome.message === formerLimit;

const refusal: Outcome = { stage: "check", message: currentLimit };

export const afterOutcome = (
  before: Outcome,
  oracle: Outcome,
  depth: number | null,
): Outcome => {
  if (!formerRefusal(before)) return before;
  return depth !== null && depth >= depthLimit ? refusal : oracle;
};

const derive = (input: DepthSource, records: Records0119): Expectation => {
  const before = recorded(records.before, input.name);
  const oracle = recorded(records.oracle, input.name);
  const cell = (frontend: Frontend): Cell => ({
    v1: before[frontend].v1,
    v2: afterOutcome(before[frontend].v2, oracle[frontend].v2, input.depth),
  });
  return { ts: cell("ts"), rust: cell("rust") };
};

export const overriddenInputs = (): readonly DepthSource[] =>
  sources0075()
    .filter((input) =>
      (overriddenNames as readonly string[]).includes(input.name),
    )
    .map((input) => ({ ...input, depth: null }));

export const depthCases = (records: Records0119): readonly DepthCase[] =>
  [...sources0119(), ...overriddenInputs()].map((input) => ({
    ...input,
    expect: derive(input, records),
  }));
