import type { FrontendCase } from "./cases.js";

const recursiveFunction = String.raw`F[i \in 0..7] == IF i=0 THEN 7 ELSE IF i=1 THEN 1 \div 0 ELSE F[0]
Alias == F
Id(f) == f`;
const lazyCase = (
  name: string,
  expression: string,
  expected: FrontendCase["expected"] = "complete",
  declarations = recursiveFunction,
): FrontendCase => ({
  name,
  family: "lazy",
  expected,
  irVersion: 2,
  source: `---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\n${declarations}\nInit == x=(${expression})\nNext == UNCHANGED x\n====`,
  config: "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE",
});
export const lazyCases: readonly FrontendCase[] = [
  lazyCase("lazy-alias", "Alias[0]"),
  lazyCase("lazy-actual", "Id(Alias)[0]"),
  lazyCase("lazy-selected", "(IF TRUE THEN Alias ELSE 1 \\div 0)[0]"),
  lazyCase("lazy-record", "Id([fn |-> Alias, mark |-> TRUE]).fn[0]"),
  lazyCase("lazy-tuple", "Id(<<Alias, TRUE>>)[1][0]"),
  lazyCase("lazy-domain", "DOMAIN Id(Alias)"),
  lazyCase("lazy-demand-error", "Id(Alias)[1]", "evaluation-error"),
  lazyCase("lazy-equality-error", "Alias=Alias", "evaluation-error"),
  lazyCase("lazy-state-error", "Id(Alias)", "evaluation-error"),
  lazyCase("lazy-outside", "Id(Alias)[8]", "evaluation-error"),
  lazyCase(
    "lazy-unused-loop",
    "Id(Alias)[0]",
    "complete",
    String.raw`F[i \in {0,1}] == IF i=0 THEN 7 ELSE F[1]
Alias == F
Id(f) == f`,
  ),
  lazyCase(
    "lazy-demand-loop",
    "Id(Alias)[1]",
    "bounded",
    String.raw`F[i \in {0,1}] == IF i=0 THEN 7 ELSE F[1]
Alias == F
Id(f) == f`,
  ),
  lazyCase(
    "lazy-capture",
    "Get(7)[0]",
    "complete",
    String.raw`Get(delta) == LET F[i \in {0,1}] == IF i=0 THEN delta ELSE F[1] IN F`,
  ),
  lazyCase(
    "lazy-old",
    String.raw`([<<7>> EXCEPT ![1] = Id([i \in {0,1} |-> IF i=0 THEN @ ELSE 1 \div 0])[0]])[1]`,
  ),
  lazyCase(
    "lazy-pattern",
    "Id(Alias)[<<0,0>>]",
    "complete",
    String.raw`F[<<i,j>> \in {<<0,0>>,<<1,1>>}] == IF i=0 THEN 7 ELSE F[<<1,1>>]
Alias == F
Id(f) == f`,
  ),
  lazyCase(
    "lazy-product",
    "Id(Alias)[0,0]",
    "complete",
    String.raw`F[i \in {0,1}, j \in {0,1}] == IF i+j=0 THEN 7 ELSE F[1,1]
Alias == F
Id(f) == f`,
  ),
  lazyCase(
    "lazy-materialize",
    "Id(Alias)",
    "complete",
    String.raw`F[i \in 0..3] == IF i=0 THEN 0 ELSE F[i-1]+1
Alias == F
Id(f) == f`,
  ),
  {
    name: "lazy-prime",
    family: "lazy",
    expected: "complete",
    irVersion: 2,
    source: String.raw`---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
F[i \in {0,1}] == IF i=0 THEN x ELSE F[1]
Id(f) == f
Init == x=0
Next == x'=(x+1)%3 /\ Id(F)[0]=x /\ Id(F')[0]=x'
====`,
    config: "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE",
  },
];

export const temporalCases: readonly FrontendCase[] = [false, true].map(
  (fault): FrontendCase => ({
    name: `temporal-${fault ? "fault" : "normal"}`,
    family: "template",
    expected: fault ? "temporal-violation" : "complete",
    irVersion: 2,
    source: `---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\nT(target) == []<>(x=target)\nBoth(target) == T(target) /\\ T(target)\nInit == x=0\nNext == x'=(x+1)%3\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nLive == Both(${fault ? 4 : 0})\n====`,
    config: "SPECIFICATION Spec\nPROPERTY Live\nCHECK_DEADLOCK FALSE",
  }),
);
