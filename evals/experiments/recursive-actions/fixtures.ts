import { writeFile } from "node:fs/promises";
import type { TlaSourceInput } from "../../../packages/idealis/src/contracts.js";

interface Case extends TlaSourceInput {
  readonly name: string;
  readonly family: string;
  readonly status: string;
  readonly states: number;
  readonly generated?: number;
}

const cases: Case[] = [];
const updateLast = (update: (input: Case) => Case) => {
  const input = cases.at(-1);
  if (!input) throw new Error("No recursive action case to update");
  cases[cases.length - 1] = update(input);
};
const add = (
  name: string,
  family: string,
  declarations: string,
  next: string,
  states: number,
  generated: number,
  variables = "x",
  initial = "x=0",
  safe = "TRUE",
) =>
  cases.push({
    name,
    family,
    source: `---- MODULE Differential ----\nEXTENDS Naturals, Sequences\nVARIABLES ${variables}\n${declarations}\nInit == ${initial}\nNext == ${next}\nSafe == ${safe}\n====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    irVersion: 2,
    status: "complete",
    states,
    generated,
  });
const tail =
  "RECURSIVE Down(_)\nDown(n) == IF n=0 THEN x'=(x+1)%3 ELSE Down(n-1)";
for (const depth of [0, 4, 12])
  add(`tail-${depth}`, "tail", tail, `Down(${depth})`, 3, 4);
add("inline-control", "control", "", "x'=(x+1)%3", 3, 4);
add(
  "mutual",
  "mutual",
  "RECURSIVE Down(_), Other(_)\nDown(n) == IF n=0 THEN x'=(x+1)%3 ELSE Other(n-1)\nOther(n) == Down(n)",
  "Down(4)",
  3,
  4,
);
add(
  "branching",
  "branching",
  String.raw`RECURSIVE Down(_)
Down(n) == IF n=0 THEN (\E delta \in 1..2: x'=(x+delta)%3) ELSE Down(n-1) \/ x'=x`,
  "Down(3)",
  3,
  16,
);
add(
  "continuation",
  "continuation",
  tail,
  String.raw`\E choice \in {0,1}: Down(3) /\ y'=choice /\ x'=(x+1)%3`,
  6,
  13,
  "x,y",
  String.raw`x=0 /\ y=0`,
);
add(
  "captured",
  "capture",
  "",
  String.raw`\E delta \in 1..2: LET RECURSIVE Down(_) Down(n) == IF n=0 THEN x'=(x+delta)%3 ELSE Down(n-1) IN Down(3)`,
  3,
  7,
);
add(
  "prime-actual",
  "prime",
  "RECURSIVE Copy(_,_)\nCopy(n,p) == IF n=0 THEN y'=p ELSE Copy(n-1,p)",
  String.raw`x'=(x+1)%3 /\ Copy(3,x')`,
  3,
  4,
  "x,y",
  String.raw`x=0 /\ y=0`,
  "x=y",
);
add(
  "unchanged-target",
  "target",
  "RECURSIVE Keep(_,_)\nKeep(v,n) == IF n=0 THEN UNCHANGED v ELSE Keep(v,n-1)",
  String.raw`(\E delta \in 1..2: x'=(x+delta)%3) /\ Keep(y,3)`,
  3,
  7,
  "x,y",
  String.raw`x=0 /\ y=7`,
  "y=7",
);
updateLast((input) => ({ ...input, status: "error" }));
add(
  "target-capture",
  "target",
  "Outer(v) == LET RECURSIVE Keep(_) Keep(n) == IF n=0 THEN UNCHANGED v ELSE Keep(n-1) IN Keep(3)",
  String.raw`(\E delta \in 1..2: x'=(x+delta)%3) /\ Outer(y)`,
  3,
  7,
  "x,y",
  String.raw`x=0 /\ y=7`,
  "y=7",
);

add(
  "except",
  "except",
  "RECURSIVE Write(_,_)\nWrite(n,k) == IF n=0 THEN x'=[x EXCEPT ![k]=(@+1)%2] ELSE Write(n-1,k)",
  String.raw`\E key \in 1..2: Write(3,key)`,
  4,
  9,
  "x",
  "x = <<0,0>>",
);
add(
  "old-actual",
  "old",
  "RECURSIVE Assign(_,_)\nAssign(n,p) == IF n=0 THEN x'=[<<1>> EXCEPT ![1]=@+p][1] ELSE Assign(n-1,p)",
  "UNCHANGED x",
  1,
  2,
  "x",
  "x=0",
  String.raw`[<<2>> EXCEPT ![1]=ENABLED (x'=3 /\ Assign(2,@))][1]`,
);
add(
  "enabled-true",
  "enabled",
  tail,
  "UNCHANGED x",
  1,
  2,
  "x",
  "x=0",
  "ENABLED Down(3)",
);
add(
  "enabled-false",
  "enabled",
  tail,
  "UNCHANGED x",
  1,
  2,
  "x",
  "x=0",
  String.raw`~ENABLED (x'=2 /\ Down(3))`,
);
add(
  "conflicting-assignment",
  "conflict",
  tail,
  String.raw`x'=2 /\ Down(3)`,
  1,
  1,
);
add(
  "higher-order",
  "higher",
  `${tail}\nApply(A(_), n) == A(n)`,
  "Apply(Down,4)",
  3,
  4,
);
add(
  "recursive-init",
  "initial",
  "RECURSIVE Seed(_)\nSeed(n) == IF n=0 THEN x \\in {0,1} ELSE Seed(n-1)",
  "x'=(x+1)%3",
  3,
  5,
  "x",
  "Seed(4)",
);
add(
  "zero-arity-next",
  "initial",
  "RECURSIVE Next\nUnused == TRUE",
  "IF x=0 THEN x'=1 ELSE Next",
  2,
  2,
);
updateLast((input) => ({ ...input, status: "bounded" }));
add(
  "action-property",
  "property",
  `${tail}\nP == [][Down(3)]_x`,
  "Down(3)",
  3,
  4,
);
updateLast((input) => ({
  ...input,
  config: `${input.config}\nPROPERTY P`,
}));
add(
  "accumulator",
  "accumulator",
  "RECURSIVE Walk(_,_)\nWalk(n,acc) == IF n=0 THEN x'=acc ELSE Walk(n-1,[acc EXCEPT ![n]=(@+1)%2])",
  "Walk(2,x)",
  2,
  3,
  "x",
  "x = <<0,0>>",
);
add(
  "module-instance",
  "module",
  "I == INSTANCE Counter\nDown(n) == I!Down(n)",
  "Down(3)",
  3,
  4,
);
updateLast((input) => ({
  ...input,
  modules: {
    Counter:
      "---- MODULE Counter ----\nEXTENDS Naturals\nVARIABLE x\nRECURSIVE Down(_)\nDown(n) == IF n=0 THEN x'=(x+1)%3 ELSE Down(n-1)\n====",
  },
}));
add(
  "multi-argument",
  "arguments",
  "RECURSIVE Down(_,_,_)\nDown(n,a,b) == IF n=0 THEN x'=(x+a+b)%257 ELSE Down(n-1,a,b)",
  "Down(8,1,1)",
  257,
  258,
);
add(
  "wide",
  "parallel",
  "RECURSIVE Down(_)\nDown(n) == IF n=0 THEN x' \\in 0..127 ELSE Down(n-1)",
  "Down(3)",
  128,
  16385,
);
add("invariant-fault", "violation", tail, "Down(3)", 2, 2, "x", "x=0", "x=0");
updateLast((input) => ({
  ...input,
  status: "invariant-violation",
}));
add(
  "nontermination",
  "nontermination",
  "RECURSIVE Down(_)\nDown(n) == IF n=0 THEN x'=x ELSE Down(n+1)",
  "Down(1)",
  1,
  1,
);
updateLast((input) => ({ ...input, status: "bounded" }));

await writeFile(
  "crates/rtla-compiler/tests/recursive-actions.json",
  `${JSON.stringify(cases, null, 2)}\n`,
);
