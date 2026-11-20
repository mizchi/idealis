import type { TlaSourceInput } from "../../../packages/idealis/src/contracts.js";

export interface ActionCase extends TlaSourceInput {
  readonly irVersion: 2;
  readonly name: string;
  readonly family: string;
  readonly status: string;
  readonly states: number;
  readonly generated: number;
}

export const generatedCases = (
  seed: number,
  count: number,
): readonly ActionCase[] =>
  Array.from({ length: count }, (_, index) => {
    const value =
      (Math.imul(seed, 1664525) + Math.imul(index + 1, 1013904223)) >>> 0;
    const depth = 1 + ((value >>> 7) % 6);
    const width = 2 + ((value >>> 15) % 5);
    const mode = value % 4;
    const declarations =
      mode === 2
        ? `RECURSIVE Down(_), Other(_)\nDown(n) == IF n=0 THEN x'=(x+1)%${width} ELSE Other(n-1)\nOther(n) == Down(n)`
        : mode === 0
          ? `RECURSIVE Down(_)\nDown(n) == IF n=0 THEN x'=(x+1)%${width} ELSE Down(n-1)`
          : mode === 3
            ? `RECURSIVE Apply(_,_)\nApply(F(_),n) == IF n=0 THEN F((x+1)%${width}) ELSE Apply(F,n-1)\nAssign(v) == x'=v`
            : "";
    const next =
      mode === 1
        ? String.raw`\E delta \in {1}: LET RECURSIVE Down(_) Down(n) == IF n=0 THEN x'=(x+delta)%${width} ELSE Down(n-1) IN Down(${depth})`
        : mode === 3
          ? `Apply(Assign,${depth})`
          : `Down(${depth})`;
    return [false, true].map(
      (fault) =>
        ({
          name: `generated-${seed}-${index}-${fault ? "fault" : "normal"}`,
          family: `generated-${mode}`,
          source: `---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\n${declarations}\nInit == x=0\nNext == ${next}\nSafe == x<${width - Number(fault)}\n====`,
          config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
          irVersion: 2,
          status: fault ? "invariant-violation" : "complete",
          states: width,
          generated: fault ? width : width + 1,
        }) satisfies ActionCase,
    );
  }).flat();
