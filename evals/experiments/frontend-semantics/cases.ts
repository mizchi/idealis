import type { TlaSourceInput } from "../../../packages/idealis/src/contracts.js";

export interface FrontendCase extends TlaSourceInput {
  readonly name: string;
  readonly family: "level" | "lazy" | "template";
  readonly expected:
    | "complete"
    | "input-error"
    | "evaluation-error"
    | "bounded"
    | "invariant-violation"
    | "temporal-violation";
  readonly irVersion: 2;
}

export const levelCases: readonly FrontendCase[] = [
  ["double-prime", "Bad == (x')'", false],
  ["unchanged-action", "Bad == UNCHANGED (x')", false],
  ["enabled-temporal", "Bad == ENABLED ([](x=0))", false],
  ["tuple-temporal", "Bad == <<[](x=0)>>", false],
  ["set-temporal", "Bad == {[](x=0)}", false],
  ["equality-temporal", "Bad == ([](x=0)) = TRUE", false],
  ["subscript-action", "Bad == [x'=x]_(x')", false],
  ["fairness-subscript", "Bad == WF_(x')(x'=x)", false],
  ["formal-transitive", "P(a) == a'\nQ(b) == P(b)\nBad == Q(x')", false],
  ["let-formal", "Bad == LET P(a) == a' IN P(x')", false],
  [
    "recursive-temporal-actual",
    "RECURSIVE P(_)\nP(a) == IF TRUE THEN a ELSE P(a)\nBad == P([]TRUE)",
    false,
  ],
  ["recursive-prime-formal", "RECURSIVE P(_)\nP(a) == a'", false],
  ["ignored-actual", "P(a) == TRUE\nGood == P([]TRUE)'", true],
  ["formal-state", "P(a) == a'\nGood == P(x)", true],
  ["enabled-weight", "P(a) == ENABLED a\nGood == P(x'=x)'", true],
  ["let-state", "Good == LET P(a) == a' IN P(x)", true],
  [
    "recursive-action-actual",
    "RECURSIVE P(_)\nP(a) == IF TRUE THEN a ELSE P(a)\nGood == P(x')",
    true,
  ],
].map(([name, definitions, valid]) => {
  if (
    typeof name !== "string" ||
    typeof definitions !== "string" ||
    typeof valid !== "boolean"
  )
    throw new Error("Invalid level fixture");
  return {
    name,
    family: "level",
    expected: valid ? "complete" : "input-error",
    irVersion: 2,
    source: `---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\n${definitions}\nInit == x=0\nNext == x'=x\n====`,
    config: "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE",
  };
});

export const benchmarkCases: readonly FrontendCase[] = [
  ...[4, 6].map((depth): FrontendCase => ({
    name: `action-repeat-${depth}`,
    family: "template",
    expected: "complete",
    irVersion: 2,
    source: `---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\nA0(v) == x'=v\n${Array.from({ length: depth }, (_, index) => `A${index + 1}(v) == A${index}(v) \\/ A${index}(v)`).join("\n")}\nInit == x=0\nNext == A${depth}((x+1)%4)\n====`,
    config: "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE",
  })),
  {
    name: "action-capture",
    family: "template",
    expected: "complete",
    irVersion: 2,
    source: String.raw`---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == \E delta \in {1,2}: LET Step(v) == x'=(x+delta+v)%4 IN Step(0) \/ Step(0) \/ Step(0) \/ Step(0)
====`,
    config: "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE",
  },
  ...[3, 5].map((depth): FrontendCase => ({
    name: `temporal-repeat-${depth}`,
    family: "template",
    expected: "complete",
    irVersion: 2,
    source: `---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\nT0(limit) == [](x<limit)\n${Array.from({ length: depth }, (_, index) => `T${index + 1}(limit) == T${index}(limit) /\\ T${index}(limit)`).join("\n")}\nInit == x=0\nNext == x'=(x+1)%4\nLive == T${depth}(4)\n====`,
    config: "INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE",
  })),
  {
    name: "scalar-control",
    family: "template",
    expected: "complete",
    irVersion: 2,
    source:
      "---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=(x+1)%4\n====",
    config: "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE",
  },
  {
    name: "lazy-control",
    family: "lazy",
    expected: "complete",
    irVersion: 2,
    source:
      "---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\nF[i \\in 0..15] == IF i=0 THEN 1 ELSE IF i=1 THEN 1 \\div 0 ELSE F[0]\nAlias == F\nId(f) == f\nInit == x=0\nNext == x'=(x+Id(Alias)[0])%4\n====",
    config: "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE",
  },
];
