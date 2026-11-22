import assert from "node:assert/strict";
import {
  type Cell,
  type Expectation,
  type Frontend,
  type Outcome,
  parserLimit,
} from "./outcome.js";
import { type Source, sources } from "./sources.js";

export interface ChainCase extends Source {
  readonly expect: Expectation;
}

export type Baseline = Readonly<Record<string, Expectation>>;

const parserRefusal = (outcome: Outcome): boolean =>
  outcome.stage === "compile" && outcome.message.startsWith(parserLimit);

const recorded = (baseline: Baseline, name: string): Expectation => {
  const found = baseline[name];
  assert.ok(found, `baseline for ${name}`);
  return found;
};

const withoutWork = (outcome: Outcome): Outcome =>
  outcome.stage === "result" ? { ...outcome, work: null } : outcome;

const cell = (baseline: Baseline, input: Source, frontend: Frontend): Cell => {
  const own = recorded(baseline, input.name)[frontend];
  const partner = input.equivalent ?? input.semantic;
  if (partner === null || parserRefusal(own.v2)) return own;
  const same = recorded(baseline, partner)[frontend].v2;
  return {
    v1: own.v1,
    v2: input.equivalent === null ? withoutWork(same) : same,
  };
};

export const chainCases = (baseline: Baseline): readonly ChainCase[] =>
  sources().map((input) => ({
    ...input,
    expect: {
      ts: cell(baseline, input, "ts"),
      rust: cell(baseline, input, "rust"),
    },
  }));
