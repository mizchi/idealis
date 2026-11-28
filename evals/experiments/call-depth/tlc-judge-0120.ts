import assert from "node:assert/strict";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import type { Outcome } from "../nesting-chains/outcome.js";
import type { TlcResult } from "./tlc-0120.js";

export interface JudgedCell {
  readonly outcome: Outcome;
  readonly summary: {
    readonly states: number;
    readonly generated: number;
  } | null;
  readonly trace: unknown;
}

export interface Judgement {
  readonly label: string;
  readonly agrees: boolean | null;
}

const traceValues = (trace: unknown): string[][] => {
  assert.ok(Array.isArray(trace));
  return trace.map((step) => {
    assert.ok(isRecord(step) && Array.isArray(step.values));
    return step.values.map((value) => {
      assert.ok(isRecord(value));
      return String(value.value);
    });
  });
};

const reachesVerdict = (verdict: string): boolean =>
  verdict === "violation" || verdict === "division-by-zero";

const typedLimit = (message: string): boolean =>
  /nesting exceeds \d+$/.test(message);

export interface JudgeContext {
  readonly oracleAfterHits: number | null;
  readonly before: JudgedCell | null;
}

const failingBoundReasons: ReadonlySet<string> = new Set(["states", "work"]);

const noContext: JudgeContext = { oracleAfterHits: null, before: null };

const changedOnCompleteCase = (
  side: "before" | "after",
  expected: Pick<TlcResult, "verdict">,
  cell: JudgedCell,
  context: JudgeContext,
): string | null => {
  if (
    side !== "after" ||
    expected.verdict !== "complete" ||
    context.oracleAfterHits !== 0
  )
    return null;
  const outcome = cell.outcome;
  if (outcome.stage === "check" && typedLimit(outcome.message))
    return "depth-limit refusal on a case the change does not reach 256 for";
  if (outcome.stage === "result" && outcome.status === "bounded")
    return failingBoundReasons.has(outcome.reason)
      ? `bounded by ${outcome.reason} on a case the change does not reach 256 for`
      : null;
  if (outcome.stage === "compile") {
    const previous = context.before?.outcome;
    const sameAsBefore =
      previous?.stage === "compile" && previous.message === outcome.message;
    return sameAsBefore
      ? null
      : "compile refusal that before did not give, on a case the change does not reach 256 for";
  }
  return null;
};

export const judge = (
  side: "before" | "after",
  expected: Pick<TlcResult, "verdict" | "states" | "generated" | "trace">,
  cell: JudgedCell,
  context: JudgeContext = noContext,
): Judgement => {
  const outcome = cell.outcome;
  const changed = changedOnCompleteCase(side, expected, cell, context);
  if (changed !== null) return { label: `${changed} (DIFFERS)`, agrees: false };
  if (outcome.stage === "compile")
    return { label: `compile refusal: ${outcome.message}`, agrees: null };
  if (outcome.stage === "check") {
    if (outcome.message === "Integer division by zero") {
      const same = expected.verdict === "division-by-zero";
      return {
        label: same
          ? "error: division by zero (matches TLC)"
          : "error: division by zero (DIFFERS)",
        agrees: same,
      };
    }
    if (typedLimit(outcome.message)) {
      const unexpected = side === "after" && reachesVerdict(expected.verdict);
      return {
        label: unexpected
          ? `typed refusal: depth limit where TLC reaches ${expected.verdict} (DIFFERS)`
          : "typed refusal: depth limit",
        agrees: unexpected ? false : null,
      };
    }
    return { label: `error: ${outcome.message} (DIFFERS)`, agrees: false };
  }
  if (outcome.status === "complete") {
    const same =
      expected.verdict === "complete" &&
      cell.summary?.states === expected.states &&
      cell.summary?.generated === expected.generated;
    return {
      label: same ? "complete (matches TLC)" : "complete (DIFFERS)",
      agrees: same,
    };
  }
  if (outcome.status === "invariant-violation") {
    const rtlcTrace = traceValues(cell.trace);
    const tlcTrace = expected.trace.map((state) => Object.values(state));
    const same =
      expected.verdict === "violation" &&
      cell.summary?.states === expected.states &&
      cell.summary?.generated === expected.generated &&
      JSON.stringify(rtlcTrace) === JSON.stringify(tlcTrace);
    return {
      label: same
        ? "violation (matches TLC states, generated, every trace state)"
        : "violation (DIFFERS)",
      agrees: same,
    };
  }
  const unexpected = side === "after" && reachesVerdict(expected.verdict);
  return {
    label: unexpected
      ? `bounded ${outcome.reason} where TLC reaches ${expected.verdict} (DIFFERS)`
      : `bounded ${outcome.reason}`,
    agrees: unexpected ? false : null,
  };
};

if (process.argv[1]?.endsWith("tlc-judge-0120.js")) {
  assert.deepEqual(process.argv.slice(2), ["--self-test"]);
  const expectedOf = (verdict: string) => ({
    verdict,
    states: 17,
    generated: 17,
    trace: [{ x: "0" }, { x: "16" }],
  });
  const check = (message: string): JudgedCell => ({
    outcome: { stage: "check", message },
    summary: null,
    trace: null,
  });
  const cases: readonly [
    string,
    "before" | "after",
    string,
    JudgedCell,
    boolean | null,
  ][] = [
    [
      "unexpected error on a violation case (after)",
      "after",
      "violation",
      check("Missing lexical call frame"),
      false,
    ],
    [
      "unexpected error on a violation case (before)",
      "before",
      "violation",
      check("Missing lexical call frame"),
      false,
    ],
    [
      "unexpected 256 refusal on a division-by-zero case (after)",
      "after",
      "division-by-zero",
      check("Safety evaluation nesting exceeds 256"),
      false,
    ],
    [
      "256 refusal on a violation case before the change",
      "before",
      "violation",
      check("Safety evaluation nesting exceeds 256"),
      null,
    ],
    [
      "256 refusal on a complete case",
      "after",
      "complete",
      check("Safety evaluation nesting exceeds 256"),
      null,
    ],
    [
      "unexpected error on a complete case",
      "after",
      "complete",
      check("Missing lexical call frame"),
      false,
    ],
    [
      "division by zero where TLC completes",
      "after",
      "complete",
      check("Integer division by zero"),
      false,
    ],
    [
      "division by zero where TLC agrees",
      "after",
      "division-by-zero",
      check("Integer division by zero"),
      true,
    ],
    [
      "bounded where TLC reaches a violation (after)",
      "after",
      "violation",
      {
        outcome: {
          stage: "result",
          status: "bounded",
          reason: "evaluation",
          work: 1,
        },
        summary: null,
        trace: null,
      },
      false,
    ],
    [
      "bounded where TLC completes",
      "after",
      "complete",
      {
        outcome: {
          stage: "result",
          status: "bounded",
          reason: "evaluation",
          work: 1,
        },
        summary: null,
        trace: null,
      },
      null,
    ],
  ];
  for (const [title, side, verdict, cell, agrees] of cases)
    assert.equal(judge(side, expectedOf(verdict), cell).agrees, agrees, title);
  const bounded = (reason: string): JudgedCell => ({
    outcome: { stage: "result", status: "bounded", reason, work: 1 },
    summary: null,
    trace: null,
  });
  const compile = (message: string): JudgedCell => ({
    outcome: { stage: "compile", message },
    summary: null,
    trace: null,
  });
  const complete: JudgedCell = {
    outcome: {
      stage: "result",
      status: "complete",
      states: 17,
      generated: 17,
      work: null,
    },
    summary: { states: 17, generated: 17 },
    trace: null,
  };
  const none: JudgeContext = { oracleAfterHits: 0, before: complete };
  const hit: JudgeContext = { oracleAfterHits: 3, before: complete };
  const refusedBefore: JudgeContext = {
    oracleAfterHits: 0,
    before: compile("expression Apply %"),
  };
  const limit = check("Safety evaluation nesting exceeds 256");
  const contextCases: readonly [
    string,
    JudgedCell,
    JudgeContext,
    boolean | null,
  ][] = [
    ["bounded by work, 0 hits, TLC complete", bounded("work"), none, false],
    ["bounded by work, hits, TLC complete", bounded("work"), hit, null],
    ["bounded by states, 0 hits, TLC complete", bounded("states"), none, false],
    ["bounded by states, hits, TLC complete", bounded("states"), hit, null],
    [
      "bounded by states, 0 hits, no before cell",
      bounded("states"),
      { oracleAfterHits: 0, before: null },
      false,
    ],
    [
      "bounded by states, hits unknown, TLC complete",
      bounded("states"),
      { oracleAfterHits: null, before: complete },
      null,
    ],
    ["256 refusal, 0 hits, TLC complete", limit, none, false],
    ["256 refusal, hits, TLC complete", limit, hit, null],
    [
      "compile refusal new on after, 0 hits",
      compile("recursion limit"),
      none,
      false,
    ],
    [
      "compile refusal equal to before, 0 hits",
      compile("expression Apply %"),
      refusedBefore,
      null,
    ],
    ["compile refusal, hits", compile("recursion limit"), hit, null],
    ["bounded by evaluation, 0 hits", bounded("evaluation"), none, null],
    ["complete matching TLC, 0 hits", complete, none, true],
  ];
  for (const [title, cell, context, agrees] of contextCases)
    assert.equal(
      judge("after", expectedOf("complete"), cell, context).agrees,
      agrees,
      title,
    );
  for (const reason of ["work", "states"])
    assert.equal(
      judge("before", expectedOf("complete"), bounded(reason), none).agrees,
      null,
      `before is never judged by the change rule (${reason})`,
    );
  for (const verdict of ["violation", "division-by-zero"])
    assert.equal(
      judge("after", expectedOf(verdict), bounded("states"), none).agrees,
      false,
      `bounded by states where TLC reaches ${verdict}`,
    );
  console.log(
    `${cases.length + contextCases.length + 4} judge self-test cases passed`,
  );
}
