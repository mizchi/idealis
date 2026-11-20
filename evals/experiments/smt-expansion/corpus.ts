import type { Input } from "../smt-subset/corpus.js";

export const integer = (value: bigint | number): string =>
  BigInt(value) < 0n ? `(- ${-BigInt(value)})` : String(value);
const input = (
  name: string,
  assertions: string,
  expected: "sat" | "unsat",
): Input => ({
  name,
  family: "smt-expansion",
  integers: ["x", "y"],
  booleans: ["b"],
  source: `(declare-const x Int)(declare-const y Int)(declare-const b Bool)${assertions}`,
  expected,
  iterations: 1,
});
export const handCases = (): readonly Input[] => [
  input(
    "existing-idl-control",
    "(assert (< x y))(assert (<= (- y x) 2))",
    "sat",
  ),
  input(
    "positive-rounding",
    "(assert (<= (* 2 x) 7))(assert (> x 3))",
    "unsat",
  ),
  input(
    "negative-rounding-unsat",
    "(assert (<= (* 2 x) (- 3)))(assert (>= x (- 1)))",
    "unsat",
  ),
  input(
    "negative-rounding-sat",
    "(assert (<= (* 2 x) (- 3)))(assert (>= x (- 2)))",
    "sat",
  ),
  input(
    "negative-factor-strict",
    "(assert (< (* (- 3) x) 4))(assert (<= x (- 2)))",
    "unsat",
  ),
  input("nondivisible-equality", "(assert (= (* 2 x) 7))", "unsat"),
  input("zero-factor", "(assert (= (* 0 x) 0))", "sat"),
  input(
    "scaled-difference",
    "(assert (<= (* 3 (- x y)) (- 4)))(assert (>= (- x y) (- 1)))",
    "unsat",
  ),
  input(
    "large-factor",
    "(assert (>= (* 10000000000000000000000000000000000000000 x) 1))(assert (<= x 0))",
    "unsat",
  ),
  input(
    "integer-ite-let",
    "(assert b)(assert (let ((v (ite b (+ x 1) (- x 1)))) (= (* 2 v) 6)))",
    "sat",
  ),
  input("integer-ite-negation", "(assert (not (> (ite b 5 6) 3)))", "unsat"),
  input(
    "nested-integer-ite",
    "(assert (= x (ite (> y 0) (ite b 2 3) 4)))(assert (> y 0))(assert b)(assert (not (= x 2)))",
    "unsat",
  ),
];
export const generated = (seed: number, count: number): readonly Input[] => {
  let state = seed >>> 0;
  const next = () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state;
  };
  return Array.from({ length: count }, (_, index) => {
    const difference = BigInt((next() % 31) - 15);
    const offset = BigInt((next() % 11) - 5);
    const factor = BigInt((next() % 19) - 9);
    const positive = ((next() >>> 16) & 1) === 0;
    const value = factor * (difference + (positive ? offset : -offset));
    const bound = value + BigInt((next() % 5) - 2);
    const operator = ["<", "<=", ">", ">=", "=", "distinct"][
      (next() >>> 16) % 6
    ];
    const truth =
      operator === "<"
        ? value < bound
        : operator === "<="
          ? value <= bound
          : operator === ">"
            ? value > bound
            : operator === ">="
              ? value >= bound
              : operator === "="
                ? value === bound
                : value !== bound;
    const choice = `(ite b (+ x ${integer(offset)}) (- x ${integer(offset)}))`;
    const term =
      index % 2 === 0
        ? `(* ${integer(factor)} (- ${choice} y))`
        : `(- (* ${integer(factor)} ${choice}) (* ${integer(factor)} y))`;
    return input(
      `generated-${seed}-${index}`,
      `(assert (<= (- x y) ${integer(difference)}))(assert (>= (- x y) ${integer(difference)}))(assert ${positive ? "b" : "(not b)"})(assert (${operator} ${term} ${integer(bound)}))`,
      truth ? "sat" : "unsat",
    );
  });
};
export const controls = [
  "(assert (and))",
  "(assert (or))",
  "(assert (not (and)))",
  "(assert (not (or)))",
  "(declare-const x Int)(declare-const y Int)(assert (<= (+ (* 2 x) (* 3 y)) 7))",
  "(declare-const x Int)(declare-const y Int)(assert (= (* x y) 2))",
  "(assert (= (ite true 1 false) 1))",
  "(push 1)",
  "(check-sat-assuming ())",
  "(declare-const x Real)",
  `${Array.from({ length: 9 }, (_, index) => `(declare-const b${index} Bool)`).join("")}(assert (<= (+ ${Array.from({ length: 9 }, (_, index) => `(ite b${index} 1 0)`).join(" ")}) 5))`,
];
