import { writeFile } from "node:fs/promises";

const cases: {
  name: string;
  family: string;
  source: string;
  config: string;
  versions: number[];
  status: string;
  states: number | null;
  modules?: Record<string, string>;
}[] = [];
const add = (
  name: string,
  family: string,
  definitions: string,
  initial: string,
  next: string,
  invariant: string,
  states: number,
  versions = [1, 2],
  imports = "Naturals",
  modules: Record<string, string> = {},
  config = "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
) =>
  cases.push({
    name,
    family,
    source: `---- MODULE Completion ----\nEXTENDS ${imports}\nVARIABLE x\n${definitions}\nInit == x = ${initial}\nNext == ${next}\nSafe == ${invariant}\n====`,
    config,
    modules,
    versions,
    status: "complete",
    states,
  });
for (const [name, definitions, next] of [
  [
    "named",
    String.raw`Apply(F(_), value) == F(value)
Inc(value) == (value+1)%3`,
    "x'=Apply(Inc,x)",
  ],
  [
    "capture",
    "Apply(F(_), value) == F(value)",
    String.raw`\E delta \in 1..2: x'=Apply(LAMBDA value: (value+delta)%3,x)`,
  ],
  [
    "forward",
    String.raw`Apply(F(_), value) == F(value)
Forward(G(_), value) == Apply(G,value)`,
    "x'=Forward(LAMBDA value: (value+1)%3,x)",
  ],
  [
    "builtin",
    "Combine(F(_,_), left, right) == F(left,right)",
    "x'=Combine(+,x,1)%3",
  ],
  [
    "action",
    String.raw`Run(A(_), value) == A(value)
Assign(value) == x'=value`,
    "Run(Assign,(x+1)%3)",
  ],
] as const)
  add(name, "higher", definitions, "0", next, String.raw`x \in 0..2`, 3);
for (const [name, definition, expression, expected] of [
  ["operand", "Op == 1+2", "Op!2", "2"],
  ["conditional", "Op(value) == IF value=0 THEN 10 ELSE 20", "Op(0)!2", "10"],
  [
    "label",
    String.raw`Op(value) == \A bound \in 0..2: label(bound)::value+bound`,
    "Op(1)!label(2)",
    "3",
  ],
  [
    "bound-arguments",
    String.raw`Op == \A bound \in 0..2: bound+1`,
    "Op!(2)",
    "3",
  ],
  [
    "bound-operator",
    String.raw`Op == \A bound \in 0..2: bound+1
Apply(F(_), value) == F(value)`,
    "Apply(Op!@,2)",
    "3",
  ],
  ["case", "Op == CASE TRUE -> 3 [] OTHER -> 4", "Op!1!2", "3"],
  [
    "let-definition",
    "Op == LET Inner(value) == value+1 IN TRUE",
    "Op!:!Inner(2)",
    "3",
  ],
  ["record", "Op == [first |-> 1, second |-> 3]", "Op!2", "3"],
] as const)
  add(
    name,
    "selection",
    definition,
    expression,
    "UNCHANGED x",
    `x=${expected}`,
    1,
  );
for (const [name, definition, expression, expected] of [
  [
    "factorial",
    String.raw`RECURSIVE Fact(_)
Fact(value) == IF value=0 THEN 1 ELSE value*Fact(value-1)`,
    "Fact(5)",
    "120",
  ],
  [
    "mutual",
    String.raw`RECURSIVE Even(_), Odd(_)
Even(value) == IF value=0 THEN TRUE ELSE Odd(value-1)
Odd(value) == IF value=0 THEN FALSE ELSE Even(value-1)`,
    "IF Even(6) THEN 6 ELSE 7",
    "6",
  ],
  [
    "state",
    String.raw`RECURSIVE Sum(_)
Sum(value) == IF value=0 THEN 0 ELSE value+Sum(value-1)`,
    "IF x=3 THEN Sum(x) ELSE x",
    "6",
  ],
] as const)
  add(
    name,
    "recursion",
    definition,
    "3",
    `x'=${expression}`,
    String.raw`x=3 \/ x=${expected}`,
    2,
    [2],
  );
for (const [name, definition, expression, invariant] of [
  [
    "function",
    String.raw`Factorial[value \in 0..5] == IF value=0 THEN 1 ELSE value*Factorial[value-1]`,
    "Factorial[5]",
    "x=120",
  ],
  [
    "multi-key",
    String.raw`Fn[left \in 0..2, right \in 0..2] == IF left=0 THEN right ELSE Fn[left-1,right]+1`,
    "Fn[2,1]",
    "x=3",
  ],
  [
    "tuple-key",
    String.raw`Fn[<<left,right>> \in {<<0,1>>, <<1,1>>, <<2,1>>}] == IF left=0 THEN right ELSE Fn[<<left-1,right>>]+1`,
    "Fn[<<2,1>>]",
    "x=3",
  ],
  [
    "domain",
    String.raw`Fn[value \in 0..1] == IF value=0 THEN 0 ELSE Fn[1]`,
    "DOMAIN Fn",
    "x=0..1",
  ],
  [
    "requested-key",
    String.raw`Fn[value \in 0..1] == IF value=0 THEN 0 ELSE Fn[1]`,
    "Fn[0]",
    "x=0",
  ],
] as const)
  add(
    name,
    "recursive-function",
    definition,
    expression,
    "UNCHANGED x",
    invariant,
    1,
    [2],
  );
for (const [name, definition, expression, expected] of [
  [
    "except-argument",
    String.raw`Apply(F(_), value) == F(value)
Update(value) == [<<1>> EXCEPT ![1]=@+value]`,
    "[<<2>> EXCEPT ![1]=Apply(Update,@)[1]]",
    "<<3>>",
  ],
  [
    "forwarded-select",
    "Filter(P(_), seq) == SelectSeq(seq,P)",
    "Filter(LAMBDA value: value>1,<<1,2,3>>)",
    "<<2,3>>",
  ],
  [
    "select-lambda",
    "",
    "SelectSeq(<<1,2,3>>,LAMBDA value: value>1)",
    "<<2,3>>",
  ],
  [
    "sort-lambda",
    "",
    "SortSeq(<<3,1,2>>,LAMBDA left,right: left<=right)",
    "<<1,2,3>>",
  ],
] as const)
  add(
    name,
    "callback",
    definition,
    expression,
    "UNCHANGED x",
    `x = ${expected}`,
    1,
    name === "except-argument" ? [2] : [1, 2],
    "Naturals, Sequences, TLC",
  );
const config = "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE";
const counter = String.raw`---- MODULE Counter ----
EXTENDS Naturals
CONSTANT Limit
VARIABLE value
Init == value=0
Next == value'=IF value<Limit THEN value+1 ELSE 0
Safe == value \in 0..Limit
====`;
cases.push({
  name: "instance-parameter",
  family: "module",
  source: String.raw`---- MODULE Completion ----
EXTENDS Naturals
VARIABLE x
C(limit) == INSTANCE Counter WITH Limit <- limit, value <- x
Init == C(2)!Init
Next == C(2)!Next
Safe == C(2)!Safe
====`,
  config,
  modules: { Counter: counter },
  versions: [1, 2],
  status: "complete",
  states: 3,
});
add(
  "let-instance",
  "module",
  "",
  "0",
  String.raw`\E delta \in 1..2: LET C(limit) == INSTANCE Counter WITH Limit <- limit, value <- x IN C(delta)!Next`,
  String.raw`x \in 0..2`,
  3,
  [1, 2],
  "Naturals",
  { Counter: counter },
);
for (const [name, definitions, initial, invariant, constants] of [
  [
    "nested",
    String.raw`CONSTANT N
---- MODULE Inner ----
Init == x=0
Next == x'=IF x<N THEN x+1 ELSE 0
====
I == INSTANCE Inner`,
    "0",
    String.raw`x \in 0..N`,
    "CONSTANT N=2\n",
  ],
  [
    "nested-shadow",
    String.raw`CONSTANT N
---- MODULE Inner ----
CONSTANT N
Value == N
====
I == INSTANCE Inner WITH N <- 1`,
    "I!Value",
    "x=1",
    "CONSTANT N=2\n",
  ],
  [
    "nested-operator-capture",
    String.raw`Base == 2
---- MODULE Inner ----
Value == Base
====
I == INSTANCE Inner`,
    "I!Value",
    "x=2",
    "",
  ],
  [
    "nested-extends",
    String.raw`CONSTANT N
---- MODULE First ----
Value == N
====
---- MODULE Second ----
EXTENDS First
Result == Value
====
I == INSTANCE Second`,
    "I!Result",
    "x=2",
    "CONSTANT N=2\n",
  ],
] as const)
  add(
    name,
    "module",
    definitions,
    initial,
    name === "nested" ? "I!Next" : "UNCHANGED x",
    invariant,
    name === "nested" ? 3 : 1,
    [1, 2],
    "Naturals",
    {},
    constants + config,
  );
const parameter = String.raw`---- MODULE Parameter ----
EXTENDS Naturals
CONSTANT N
Add(value) == value+N
====`;
add(
  "namespace-operator",
  "module",
  "I(increment) == INSTANCE Parameter WITH N <- increment\nApply(F(_), value) == F(value)",
  "0",
  "x'=Apply(I(1)!Add,x)%3",
  String.raw`x \in 0..2`,
  3,
  [1, 2],
  "Naturals",
  { Parameter: parameter },
);
add(
  "namespace-all-parameters",
  "module",
  "I(increment) == INSTANCE Parameter WITH N <- increment\nCombine(F(_,_), left, right) == F(left,right)",
  "0",
  "x'=Combine(I!Add,1,x)%3",
  String.raw`x \in 0..2`,
  3,
  [1, 2],
  "Naturals",
  { Parameter: parameter },
);
cases.push({
  name: "nested-capture-override",
  family: "module-rejection",
  source: String.raw`---- MODULE Completion ----
CONSTANT N
VARIABLE x
---- MODULE Inner ----
Value == N
====
I == INSTANCE Inner WITH N <- 1
Init == x=I!Value
Next == UNCHANGED x
Safe == TRUE
====`,
  config: `CONSTANT N=2\n${config}`,
  versions: [1, 2],
  status: "error",
  states: null,
});
for (const [name, safe] of [
  [
    "grouped-let-captures",
    String.raw`\A left,right \in {1,2}: LET Add(value) == left+right+value IN Add(0)=left+right`,
  ],
  [
    "grouped-lambda-captures",
    String.raw`\A left,right \in {1,2}: Apply(LAMBDA value: left+right+value,0)=left+right`,
  ],
] as const)
  add(
    name,
    "binding",
    "Apply(F(_),value) == F(value)",
    "0",
    "UNCHANGED x",
    safe,
    1,
  );
add(
  "recursive-guard",
  "recursion",
  `RECURSIVE Even(_)
Even(value) == IF value=0 THEN TRUE ELSE IF value=1 THEN FALSE ELSE Even(value-2)`,
  "0",
  String.raw`Even(2*x) /\ x'=(x+1)%3`,
  "[]Even(2*x)",
  3,
  [2],
  "Naturals",
  {},
  config.replace("INVARIANT Safe", "PROPERTY Safe"),
);
for (const [name, definition] of [
  ["label-missing-bound", String.raw`Unused == \A bound \in {1}: label::TRUE`],
  [
    "label-extra-bound",
    String.raw`Unused == \A bound \in {1}: label(other)::TRUE`,
  ],
  ["label-duplicate", "Unused == IF TRUE THEN label::1 ELSE label::2"],
  ["label-except", "Unused == [<<1>> EXCEPT ![1]=label::@]"],
  [
    "lambda-builtin",
    "Apply(F(_),value) == F(value)\nUnused == Apply(LAMBDA TRUE: TRUE,1)",
  ],
  ["recursive-wrong-arity", "RECURSIVE Unused(_,_)\nUnused(value) == value"],
] as const)
  add(name, "declaration-rejection", definition, "0", "UNCHANGED x", "TRUE", 1);
for (const [name, definition] of [
  [
    "label-nested",
    String.raw`Unused == \A outer \in {1}: first(outer):: \A inner \in {2}: second(inner)::TRUE`,
  ],
  ["label-function", String.raw`Unused[bound \in {1}] == label(bound)::bound`],
  [
    "label-lambda",
    String.raw`Apply(F(_),value) == F(value)
Unused == \A outer \in {1}: Apply(LAMBDA inner: label(outer,inner)::TRUE,2)`,
  ],
] as const)
  add(name, "declaration", definition, "0", "UNCHANGED x", "TRUE", 1);
add(
  "except-enabled-dynamic",
  "capture",
  "",
  "0",
  "UNCHANGED x",
  String.raw`x=0 /\ [<<9>> EXCEPT ![1] = LET Step(value) == @=4 /\ x'=value IN [<<4>> EXCEPT ![1] = ENABLED Step(x)][1]][1]`,
  1,
  [2],
  "Naturals, Sequences",
);
for (const [name, definition, safe] of [
  [
    "except-choose-domain",
    String.raw`Pick == CHOOSE value \in @: TRUE`,
    "Pick=9",
  ],
  [
    "except-unused-choose",
    String.raw`Pick == CHOOSE value \in {}: @=value`,
    "TRUE",
  ],
  ["except-function-head", String.raw`Fn[value \in @] == value`, "Fn[9]=9"],
  [
    "except-function-value",
    String.raw`Fn == [value \in @ |-> value]`,
    "Fn[9]=9",
  ],
] as const)
  add(
    name,
    "capture",
    "",
    "0",
    "UNCHANGED x",
    String.raw`x=0 /\ [<<{9}>> EXCEPT ![1] = LET ${definition} IN ${safe}][1]`,
    1,
    [2],
  );
add(
  "assumption-lambda",
  "higher",
  String.raw`ASSUME SelectSeq(<<1,2>>,LAMBDA value: value>1) = <<2>>`,
  "0",
  "UNCHANGED x",
  "x=0",
  1,
  [1, 2],
  "Naturals, Sequences",
);
add(
  "instance-recursive-function",
  "module",
  "I(limit) == INSTANCE Functions WITH Limit <- limit",
  "I(5)!Factorial[5]",
  "UNCHANGED x",
  "x=120",
  1,
  [2],
  "Naturals",
  {
    Functions: String.raw`---- MODULE Functions ----
EXTENDS Naturals
CONSTANT Limit
Factorial[value \in 0..Limit] == IF value=0 THEN 1 ELSE value*Factorial[value-1]
====`,
  },
);
add(
  "fold-recursive",
  "recursion",
  String.raw`RECURSIVE Add(_,_)
Add(left,right) == IF right=0 THEN left ELSE Add(left+1,right-1)`,
  "ApaFoldSeqLeft(Add,0,<<1,2,3>>)",
  "UNCHANGED x",
  "x=6",
  1,
  [2],
  "Naturals, Apalache",
);
add(
  "except-zero-arity",
  "capture",
  "",
  "0",
  "UNCHANGED x",
  String.raw`x=0 /\ [<<9>> EXCEPT ![1] = LET Step == @=9 /\ x'=x IN [<<4>> EXCEPT ![1] = ENABLED Step][1]][1]`,
  1,
  [2],
  "Naturals, Sequences",
);
add(
  "specialized-action-trace",
  "higher-action",
  "Run(A(_),value) == A(value)\nAssign(value) == x'=value",
  "0",
  "Run(Assign,(x+1)%3)",
  "x=0",
  2,
);
const actionFault = cases.at(-1);
if (actionFault) actionFault.status = "invariant-violation";
add(
  "selector-action-trace",
  "selection-action",
  "Op(value) == label:: x'=value",
  "0",
  "Op((x+1)%3)!label",
  "x=0",
  2,
);
const selectorFault = cases.at(-1);
if (selectorFault) selectorFault.status = "invariant-violation";
for (const input of cases) {
  if (
    input.name === "nested-shadow" ||
    input.name === "namespace-operator" ||
    input.family === "declaration-rejection"
  ) {
    input.family = "module-rejection";
    input.status = "error";
    input.states = null;
  }
}
await writeFile(
  "crates/rtla-compiler/tests/frontend-completion.json",
  `${JSON.stringify(cases, null, 2)}\n`,
);
