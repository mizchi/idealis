import { integer } from "../smt-expansion/corpus.js";
import type { Input } from "../smt-subset/corpus.js";
import type { Assumption } from "../smt-subset/wire.js";

export type Step =
  | { readonly kind: "assert"; readonly expression: string }
  | { readonly kind: "push"; readonly count: number }
  | { readonly kind: "pop"; readonly count: number }
  | {
      readonly kind: "check";
      readonly expected: "sat" | "unsat";
      readonly assumptions?: readonly Assumption[];
    };
export interface ScriptInput {
  readonly name: string;
  readonly source: string;
  readonly steps: readonly Step[];
  readonly snapshots: readonly Input[];
}
const script = (name: string, steps: readonly Step[]): ScriptInput => {
  const declarations = "(declare-const x Int)(declare-const b Bool)";
  const frames: string[][] = [[]];
  const commands = [declarations];
  const snapshots: Input[] = [];
  for (const step of steps) {
    if (step.kind === "assert") {
      frames[frames.length - 1]?.push(step.expression);
      commands.push(`(assert ${step.expression})`);
    } else if (step.kind === "push") {
      for (let index = 0; index < step.count; index++) frames.push([]);
      commands.push(`(push ${step.count})`);
    } else if (step.kind === "pop") {
      if (step.count >= frames.length) throw new Error("Invalid fixture scope");
      frames.splice(frames.length - step.count, step.count);
      commands.push(`(pop ${step.count})`);
    } else {
      const assumptions = step.assumptions?.map((value) =>
        value.positive ? value.symbol : `(not ${value.symbol})`,
      );
      commands.push(
        assumptions
          ? `(check-sat-assuming (${assumptions.join(" ")}))`
          : "(check-sat)",
      );
      snapshots.push({
        name: `${name}-${snapshots.length}`,
        family: "script-snapshot",
        integers: ["x"],
        booleans: ["b"],
        source:
          declarations +
          [...frames.flat(), ...(assumptions ?? [])]
            .map((expression) => `(assert ${expression})`)
            .join(""),
        expected: step.expected,
        iterations: 1,
      });
    }
  }
  return { name, source: commands.join("\n"), steps, snapshots };
};
const scenario = (
  name: string,
  value: number,
  factor: number,
  depth: number,
): ScriptInput =>
  script(name, [
    { kind: "assert", expression: "(>= x 0)" },
    { kind: "check", expected: "sat" },
    { kind: "push", count: depth },
    {
      kind: "assert",
      expression: `(= (* ${integer(factor)} x) (ite b ${integer(factor * value)} ${integer(factor * (value + 1))}))`,
    },
    {
      kind: "check",
      expected: "sat",
      assumptions: [{ symbol: "b", positive: true }],
    },
    { kind: "assert", expression: `(= x ${integer(value)})` },
    {
      kind: "check",
      expected: "sat",
      assumptions: [{ symbol: "b", positive: true }],
    },
    {
      kind: "check",
      expected: "unsat",
      assumptions: [{ symbol: "b", positive: false }],
    },
    { kind: "push", count: 1 },
    { kind: "assert", expression: `(= x ${integer(value + 1)})` },
    { kind: "check", expected: "unsat" },
    { kind: "pop", count: 1 },
    { kind: "check", expected: "sat" },
    { kind: "pop", count: depth },
    { kind: "assert", expression: "(<= x 0)" },
    {
      kind: "check",
      expected: "sat",
      assumptions: [{ symbol: "b", positive: false }],
    },
    {
      kind: "check",
      expected: "unsat",
      assumptions: [
        { symbol: "b", positive: true },
        { symbol: "b", positive: false },
      ],
    },
    { kind: "push", count: 0 },
    { kind: "pop", count: 0 },
    { kind: "check", expected: "sat", assumptions: [] },
  ]);
export const handCases = (): readonly ScriptInput[] => [
  script("existing-single-check", [
    { kind: "assert", expression: "(= x 7)" },
    { kind: "check", expected: "sat" },
  ]),
  script("no-assumption-leak", [
    {
      kind: "check",
      expected: "unsat",
      assumptions: [
        { symbol: "b", positive: true },
        { symbol: "b", positive: false },
      ],
    },
    { kind: "check", expected: "sat" },
  ]),
  scenario("negative-coefficient-nested-scopes", 2, -3, 2),
];
export const generated = (
  seed: number,
  count: number,
): readonly ScriptInput[] => {
  let state = seed >>> 0;
  const next = () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state >>> 16;
  };
  return Array.from({ length: count }, (_, index) =>
    scenario(
      `generated-${seed}-${index}`,
      1 + (next() % 50),
      (2 + (next() % 6)) * (next() & 1 ? -1 : 1),
      1 + (next() % 3),
    ),
  );
};
export const controls = [
  "(check-sat)(pop 1)",
  "(assert true)(declare-const x Int)",
  "(push 1)(declare-const x Int)",
  "(check-sat)(set-option :produce-models true)",
  "(push 01)",
  "(push -1)",
  "(push 4294967296)",
  "(push |1|)",
  "(push 257)",
  "(push 256)(push 1)",
  "(check-sat 1)",
  "(check-sat-assuming)",
  "(check-sat-assuming (true))",
  "(declare-const x Int)(check-sat-assuming (x))",
  "(declare-const b Bool)(check-sat-assuming ((not (not b))))",
  "(declare-const b Bool)(check-sat-assuming ((and b b)))",
  "(assert missing)(check-sat)",
  "(get-model)",
  "(get-value (x))",
  "(exit)",
  "(check-sat)".repeat(257),
  `(declare-const b Bool)(check-sat-assuming (${"b ".repeat(257)}))`,
];
