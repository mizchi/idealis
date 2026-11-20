export interface Input {
  readonly name: string;
  readonly family: string;
  readonly source: string;
  readonly integers: readonly string[];
  readonly booleans: readonly string[];
  readonly expected?: "sat" | "unsat";
  readonly iterations: number;
}
interface Integer {
  readonly variable: number;
  readonly offset: number;
}
type Predicate =
  | {
      readonly kind: "compare";
      readonly op: "<" | "<=" | "=" | "distinct";
      readonly left: Integer;
      readonly right: Integer;
    }
  | { readonly kind: "boolean"; readonly slot: number }
  | { readonly kind: "not"; readonly value: Predicate }
  | {
      readonly kind: "and" | "or" | "xor" | "=" | "=>";
      readonly left: Predicate;
      readonly right: Predicate;
    };
export interface Generated extends Input {
  readonly predicate: Predicate;
}
export const number = (value: number | bigint): string =>
  BigInt(value) < 0n ? `(- ${-BigInt(value)})` : String(value);
const term = (value: Integer): string =>
  value.offset === 0
    ? `x${value.variable}`
    : `(+ x${value.variable} ${number(value.offset)})`;
const render = (value: Predicate): string => {
  if (value.kind === "compare")
    return `(${value.op} ${term(value.left)} ${term(value.right)})`;
  if (value.kind === "boolean") return `b${value.slot}`;
  if (value.kind === "not") return `(not ${render(value.value)})`;
  return `(${value.kind} ${render(value.left)} ${render(value.right)})`;
};
export const evaluate = (
  value: Predicate,
  integers: readonly number[],
  booleans: readonly boolean[],
): boolean => {
  if (value.kind === "boolean") return booleans[value.slot] ?? false;
  if (value.kind === "not") return !evaluate(value.value, integers, booleans);
  if (value.kind === "compare") {
    const left = (integers[value.left.variable] ?? 0) + value.left.offset;
    const right = (integers[value.right.variable] ?? 0) + value.right.offset;
    return value.op === "<"
      ? left < right
      : value.op === "<="
        ? left <= right
        : value.op === "="
          ? left === right
          : left !== right;
  }
  const left = evaluate(value.left, integers, booleans);
  const right = evaluate(value.right, integers, booleans);
  return value.kind === "and"
    ? left && right
    : value.kind === "or"
      ? left || right
      : value.kind === "="
        ? left === right
        : value.kind === "xor"
          ? left !== right
          : !left || right;
};
export const generated = (
  seed: number,
  count: number,
): readonly Generated[] => {
  let state = seed >>> 0;
  const next = (limit: number) => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) % limit;
  };
  const expression = (depth: number): Predicate => {
    if (!depth || next(3) === 0) {
      if (next(4) === 0) return { kind: "boolean", slot: next(2) };
      return {
        kind: "compare",
        op: (["<", "<=", "=", "distinct"] as const)[next(4)] ?? "=",
        left: { variable: next(3), offset: next(5) - 2 },
        right: { variable: next(3), offset: next(5) - 2 },
      };
    }
    if (next(5) === 0) return { kind: "not", value: expression(depth - 1) };
    return {
      kind: (["and", "or", "xor", "=", "=>"] as const)[next(5)] ?? "and",
      left: expression(depth - 1),
      right: expression(depth - 1),
    };
  };
  return Array.from({ length: count }, (_, index) => {
    const predicate = expression(3);
    let satisfiable = false;
    for (let left = -2; left <= 2; left++)
      for (let middle = -2; middle <= 2; middle++)
        for (let right = -2; right <= 2; right++)
          for (let bits = 0; bits < 4; bits++)
            if (
              evaluate(
                predicate,
                [left, middle, right],
                [!!(bits & 1), !!(bits & 2)],
              )
            )
              satisfiable = true;
    return {
      name: `seed-${seed}-${index}`,
      family: "generated",
      integers: ["x0", "x1", "x2"],
      booleans: ["b0", "b1"],
      source: `(set-logic QF_IDL)${[0, 1, 2].map((slot) => `(declare-const x${slot} Int)`).join("")}(declare-const b0 Bool)(declare-const b1 Bool)${[0, 1, 2].map((slot) => `(assert (and (>= x${slot} (- 2))(<= x${slot} 2)))`).join("")}(assert ${render(predicate)})`,
      expected: satisfiable ? "sat" : "unsat",
      iterations: 1,
      predicate,
    };
  });
};
export const benchmarks = (): readonly Input[] => [
  {
    name: "ttl",
    family: "ttl",
    integers: ["ttl", "elapsed"],
    booleans: [],
    source:
      "(set-logic QF_LIA)(declare-const ttl Int)(declare-const elapsed Int)(assert (and (>= ttl 1)(<= ttl 10000)(>= elapsed 0)(<= elapsed 20000)(= elapsed ttl)))",
    expected: "sat",
    iterations: 32,
  },
  {
    name: "amount",
    family: "amount",
    integers: ["amount", "other"],
    booleans: [],
    source:
      "(set-logic QF_LIA)(declare-const amount Int)(declare-const other Int)(assert (and (> amount 0)(< amount 10)(= other 3)(= (+ amount other) 10)))",
    expected: "sat",
    iterations: 32,
  },
  ...[4, 32, 128, 512].flatMap((count) =>
    [false, true].map((unsat) => ({
      name: `chain-${count}-${unsat ? "unsat" : "sat"}`,
      family: unsat ? "chain-unsat" : "chain-sat",
      integers: Array.from({ length: count }, (_, index) => `x${index}`),
      booleans: [],
      source: `(set-logic QF_IDL)${Array.from({ length: count }, (_, index) => `(declare-const x${index} Int)`).join("")}(assert (= x0 0))${Array.from({ length: count - 1 }, (_, index) => `(assert (= x${index + 1} (+ x${index} 1)))`).join("")}${unsat ? `(assert (< x${count - 1} ${count - 1}))` : ""}`,
      expected: unsat ? ("unsat" as const) : ("sat" as const),
      iterations: count >= 128 ? 2 : 16,
    })),
  ),
  ...[4, 8, 12].flatMap((count) =>
    [false, true].map((unsat) => ({
      name: `boolean-${count}-${unsat ? "unsat" : "sat"}`,
      family: "boolean",
      integers: [],
      booleans: Array.from({ length: count }, (_, index) => `b${index}`),
      source: `(set-logic QF_IDL)${Array.from({ length: count }, (_, index) => `(declare-const b${index} Bool)`).join("")}(assert (xor ${Array.from({ length: count }, (_, index) => `b${index}`).join(" ")}))${unsat ? `(assert (not (xor ${Array.from({ length: count }, (_, index) => `b${index}`).join(" ")})))` : ""}`,
      expected: unsat ? ("unsat" as const) : ("sat" as const),
      iterations: count >= 8 ? 2 : 16,
    })),
  ),
  {
    name: "scheduler",
    family: "scheduler",
    integers: ["a", "b", "end"],
    booleans: [],
    source:
      "(set-logic QF_IDL)(declare-const a Int)(declare-const b Int)(declare-const end Int)(assert (and (>= a 0)(>= b 0)(<= end 20)(or (and (>= b (+ a 5))(>= end (+ b 4)))(and (>= a (+ b 4))(>= end (+ a 5))))))",
    expected: "sat",
    iterations: 32,
  },
];
export const handCases = (): readonly Input[] => [
  ...benchmarks().filter((input) => ["ttl", "amount"].includes(input.name)),
  {
    name: "bigint",
    family: "semantics",
    integers: ["x", "y"],
    booleans: [],
    source:
      "(declare-const x Int)(declare-const y Int)(assert (= x 10000000000000000000000000000000000000000))(assert (= y (+ x 7)))",
    expected: "sat",
    iterations: 1,
  },
  {
    name: "strict-cycle",
    family: "semantics",
    integers: ["x", "y"],
    booleans: [],
    source:
      "(declare-const x Int)(declare-const y Int)(assert (< x y))(assert (<= y x))",
    expected: "unsat",
    iterations: 1,
  },
  {
    name: "negated-equality",
    family: "semantics",
    integers: ["x"],
    booleans: [],
    source: "(declare-const x Int)(assert (not (= x 0)))(assert (<= x 0))",
    expected: "sat",
    iterations: 1,
  },
  {
    name: "parallel-let",
    family: "semantics",
    integers: ["x"],
    booleans: [],
    source:
      "(declare-const x Int)(assert (let ((x 2)(y x)) (and (= x 2)(= y 7))))",
    expected: "sat",
    iterations: 1,
  },
  {
    name: "boolean-ite",
    family: "semantics",
    integers: [],
    booleans: ["b0", "b1"],
    source:
      "(declare-const b0 Bool)(declare-const b1 Bool)(assert (ite b0 (not b1) b1))",
    expected: "sat",
    iterations: 1,
  },
  {
    name: "quoted-identifier",
    family: "semantics",
    integers: ["clock value"],
    booleans: [],
    source:
      "(declare-fun |clock value| () Int)(assert (= |clock value| (- 4)))",
    expected: "sat",
    iterations: 1,
  },
  {
    name: "contradictory-constants",
    family: "semantics",
    integers: ["x"],
    booleans: [],
    source: "(declare-const x Int)(assert (and (= x 3)(= x 4)))",
    expected: "unsat",
    iterations: 1,
  },
  {
    name: "constant-below-or",
    family: "semantics",
    integers: ["x"],
    booleans: [],
    source: "(declare-const x Int)(assert (or (= x 3)(= x 4)))(assert (= x 4))",
    expected: "sat",
    iterations: 1,
  },
];
export const controls = [
  "(assert (and))",
  "(assert (or))",
  "(declare-const x Real)",
  "(declare-const x Int)(assert (= (* x x) 2))",
  "(declare-const x Int)(declare-const y Int)(assert (= (+ x y) 7))",
  "(push 1)",
  "(declare-const x Int)(assert (= x true))",
  "(assert missing)",
  "(declare-const x Int)(assert (forall ((y Int)) (= y x)))",
  "(assert true)(check-sat)(assert false)",
  "(assert true) trailing",
  "(assert (and true)",
];
