export interface Case {
  readonly name: string;
  readonly family: "ring" | "function";
  readonly fault: boolean;
  readonly depth: number;
  readonly source: string;
  readonly config: string;
  readonly expected: "bounded" | "complete" | "invariant-violation";
}
const ring = (name: string, depth: number, fault: boolean): Case => ({
  name,
  depth,
  fault,
  family: "ring",
  source: String.raw`---- MODULE Ring ----
EXTENDS Naturals
VARIABLE
  \* @type: Int;
  x
Init == x = 0
Next == x' = IF x < 15 THEN x + 1 ELSE 0
Safe == ${fault ? "x < 8" : "x \\in 0..15"}
====`,
  config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  expected:
    fault && depth >= 8
      ? "invariant-violation"
      : depth >= 15
        ? "complete"
        : "bounded",
});
const functions = (fault: boolean): Case => ({
  name: fault ? "function-fault-4" : "function-safe-4",
  family: "function",
  depth: 4,
  fault,
  source: String.raw`---- MODULE Functions ----
EXTENDS Naturals
VARIABLE
  \* @type: Int -> Int;
  f
Init == f \in [1..8 -> 0..1]
Next == \E i \in 1..8: f' = [f EXCEPT ![i] = 1 - @]
Safe == ${fault ? "f[1] = 0" : "f \\in [1..8 -> 0..1]"}
====`,
  config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  expected: fault ? "invariant-violation" : "complete",
});
export const cases: readonly Case[] = [
  ring("ring-safe-4", 4, false),
  ring("ring-safe-15", 15, false),
  ring("ring-fault-4", 4, true),
  ring("ring-fault-8", 8, true),
  functions(false),
  functions(true),
];
