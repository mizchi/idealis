import assert from "node:assert/strict";
import type { Baseline, ChainCase } from "./cases.js";
import {
  type Cell,
  checkerLimit,
  type Expectation,
  type Frontend,
  type Outcome,
} from "./outcome.js";
import { sources0075 } from "./sources-0075.js";

export const evaluationLimit = "Safety evaluation nesting exceeds 128";

export interface Records0075 {
  readonly before0074: Baseline;
  readonly oracle0074: Baseline;
  readonly before0075: Baseline;
  readonly oracle0075: Baseline;
}

export const checkerRefusal = (outcome: Outcome): boolean =>
  outcome.stage === "check" &&
  (outcome.message === checkerLimit || outcome.message === evaluationLimit);
export const chainGroup = (group: string): boolean =>
  !group.startsWith("boundary");
const recorded = (baseline: Baseline, name: string): Expectation => {
  const found = baseline[name];
  assert.ok(found, `baseline for ${name}`);
  return found;
};

export const updatedCases = (
  cases: readonly ChainCase[],
  records: Records0075,
): readonly ChainCase[] =>
  cases.map((entry): ChainCase => {
    const before = recorded(records.before0074, entry.name);
    const oracle = recorded(records.oracle0074, entry.name);
    const cell = (frontend: Frontend): Cell => ({
      v1: entry.expect[frontend].v1,
      v2:
        chainGroup(entry.group) && checkerRefusal(before[frontend].v2)
          ? oracle[frontend].v2
          : entry.expect[frontend].v2,
    });
    return { ...entry, expect: { ts: cell("ts"), rust: cell("rust") } };
  });

export const addedCases = (records: Records0075): readonly ChainCase[] =>
  sources0075().map((input): ChainCase => {
    const before = recorded(records.before0075, input.name);
    const oracle = recorded(records.oracle0075, input.name);
    const partner =
      input.semantic === null
        ? null
        : recorded(records.before0075, input.semantic);
    const cell = (frontend: Frontend): Cell => {
      const own = before[frontend].v2;
      const v2 = !checkerRefusal(own)
        ? own
        : partner
          ? partner[frontend].v2
          : chainGroup(input.group)
            ? oracle[frontend].v2
            : own;
      return { v1: before[frontend].v1, v2 };
    };
    return { ...input, expect: { ts: cell("ts"), rust: cell("rust") } };
  });
