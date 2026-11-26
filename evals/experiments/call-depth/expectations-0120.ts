import assert from "node:assert/strict";
import type { Baseline, ChainCase } from "../nesting-chains/cases.js";
import type {
  Cell,
  Expectation,
  Frontend,
  Outcome,
} from "../nesting-chains/outcome.js";
import { overriddenInputs } from "./expectations-0119.js";
import { type DepthSource0120, sources0120 } from "./sources-0120.js";

export const depthRefusalMessage = "Safety evaluation nesting exceeds 256";

export interface Records0120 {
  readonly before: Baseline;
}

export interface DepthCase0120 extends ChainCase {
  readonly depth: number | null;
  readonly formerDepth: number | null;
}

const recorded = (baseline: Baseline, name: string): Expectation => {
  const found = baseline[name];
  assert.ok(found, `baseline for ${name}`);
  return found;
};

export const depthRefusal = (outcome: Outcome): boolean =>
  outcome.stage === "check" && outcome.message === depthRefusalMessage;

const refusal: Outcome = { stage: "check", message: depthRefusalMessage };

export const afterOutcome = (
  before: Outcome,
  input: DepthSource0120,
): Outcome => {
  if (!depthRefusal(before)) return before;
  return input.after ?? refusal;
};

const derive = (input: DepthSource0120, records: Records0120): Expectation => {
  const before = recorded(records.before, input.name);
  const cell = (frontend: Frontend): Cell => ({
    v1: before[frontend].v1,
    v2: afterOutcome(before[frontend].v2, input),
  });
  return { ts: cell("ts"), rust: cell("rust") };
};

export const inputs0120 = (): readonly DepthSource0120[] =>
  sources0120(overriddenInputs());

export const depthCases0120 = (
  records: Records0120,
): readonly DepthCase0120[] =>
  inputs0120().map(({ after: _after, ...input }) => ({
    ...input,
    expect: derive({ ...input, after: _after }, records),
  }));
