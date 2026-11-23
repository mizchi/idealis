import type { Analysis, Instance } from "./wire.js";

export interface Case {
  readonly name: string;
  readonly prelude: string;
  readonly facts: string;
  readonly query: string;
  readonly kind: "run" | "check";
  readonly scope: string;
  readonly expected: Analysis["status"];
}
export const source = (input: Case): string =>
  `${input.prelude}\nfact Assumptions { ${input.facts} }\n${input.kind === "check" ? `assert Property { ${input.query} }\nQuery: check Property` : `Query: run { ${input.query} }`} ${input.scope}\nFeasible: run {} ${input.scope}\n`;

export const handCases = (): readonly Case[] => [
  {
    name: "two-cycle",
    prelude: "sig Node { edge: set Node }",
    facts: "no iden & edge",
    query: "no iden & ^edge",
    kind: "check",
    scope: "for 2 but exactly 2 Node, 0 Int",
    expected: "counterexample",
  },
  {
    name: "dag-control",
    prelude: "sig Node { edge: set Node }",
    facts: "no iden & ^edge",
    query: "all n: Node | n not in n.^edge",
    kind: "check",
    scope: "for 2 but 0 Int",
    expected: "no-counterexample",
  },
  {
    name: "inconsistent-facts",
    prelude: "sig A {}",
    facts: "some A no A",
    query: "no A",
    kind: "check",
    scope: "for 2 but 0 Int",
    expected: "no-instance",
  },
  {
    name: "field-default-one",
    prelude: "one sig A { edge: A }",
    facts: "",
    query: "no edge",
    kind: "run",
    scope: "for 0 but 0 Int",
    expected: "no-instance",
  },
  {
    name: "lone-owner",
    prelude: "one sig Owner { resource: lone Resource } sig Resource {}",
    facts: "",
    query: "no resource",
    kind: "run",
    scope: "for 0 but 0 Int",
    expected: "instance",
  },
  {
    name: "some-field-vacuity",
    prelude: "sig Owner { resource: some Resource } sig Resource {}",
    facts: "",
    query: "no Owner",
    kind: "run",
    scope: "for 0 but 0 Int",
    expected: "instance",
  },
  {
    name: "some-field-nonempty",
    prelude: "one sig Owner { resource: some Resource } sig Resource {}",
    facts: "",
    query: "",
    kind: "run",
    scope: "for 0 but 0 Int",
    expected: "no-instance",
  },
  {
    name: "pigeonhole",
    prelude: "sig Owner { resource: Resource } sig Resource {}",
    facts: "all disj a,b: Owner | no a.resource & b.resource",
    query: "",
    kind: "run",
    scope: "for 3 but exactly 3 Owner, exactly 2 Resource, 0 Int",
    expected: "no-instance",
  },
  {
    name: "one-scope-overall-zero",
    prelude: "one sig A {}",
    facts: "",
    query: "some A",
    kind: "run",
    scope: "for 0 but 0 Int",
    expected: "instance",
  },
  {
    name: "lone-scope-overall-zero",
    prelude: "lone sig A {}",
    facts: "",
    query: "some A",
    kind: "run",
    scope: "for 0 but 0 Int",
    expected: "instance",
  },
  {
    name: "default-int-universe",
    prelude: "sig A {}",
    facts: "",
    query: "no Int",
    kind: "run",
    scope: "for 0",
    expected: "no-instance",
  },
  {
    name: "zero-bit-universe",
    prelude: "sig A {}",
    facts: "",
    query: "no univ",
    kind: "run",
    scope: "for 0 but 0 Int",
    expected: "instance",
  },
  {
    name: "int-field-witness",
    prelude: "one sig A { value: Int }",
    facts: "",
    query: "some value",
    kind: "run",
    scope: "for 0",
    expected: "instance",
  },
  {
    name: "empty-all",
    prelude: "sig A {}",
    facts: "",
    query: "all a: A | a in A",
    kind: "run",
    scope: "for 0 but 0 Int",
    expected: "instance",
  },
  {
    name: "empty-some",
    prelude: "sig A {}",
    facts: "",
    query: "some a: A | a in A",
    kind: "run",
    scope: "for 0 but 0 Int",
    expected: "no-instance",
  },
  {
    name: "one-tuples",
    prelude: "sig A {}",
    facts: "",
    query: "one a,b: A | a = b",
    kind: "run",
    scope: "for 1 but exactly 1 A, 0 Int",
    expected: "instance",
  },
  {
    name: "disjoint-ordered-tuples",
    prelude: "sig A {}",
    facts: "",
    query: "one disj a,b: A | a in A",
    kind: "run",
    scope: "for 2 but exactly 2 A, 0 Int",
    expected: "no-instance",
  },
  {
    name: "transpose-join",
    prelude: "sig N { edge: set N }",
    facts: "edge = ~edge",
    query: "edge.edge in N -> N",
    kind: "check",
    scope: "for 2 but 0 Int",
    expected: "no-counterexample",
  },
  {
    name: "reflexive",
    prelude: "sig N { edge: set N }",
    facts: "",
    query: "all n: N | n in n.*edge",
    kind: "check",
    scope: "for 2 but 0 Int",
    expected: "no-counterexample",
  },
  {
    name: "predicate",
    prelude: "sig N { edge: set N } pred Cyclic { some n: N | n in n.^edge }",
    facts: "",
    query: "Cyclic[]",
    kind: "run",
    scope: "for 2 but 0 Int",
    expected: "instance",
  },
  {
    name: "restriction",
    prelude: "sig N { edge: set N }",
    facts: "",
    query:
      "all n: N | (n <: edge) = (n -> n.edge) and (edge :> n) = ((edge.n) -> n)",
    kind: "check",
    scope: "for 2 but 0 Int",
    expected: "no-counterexample",
  },
  {
    name: "declaration-scope",
    prelude: "sig N { edge: set N }",
    facts: "",
    query: "all n: N, m: n.edge | m in N",
    kind: "check",
    scope: "for 2 but 0 Int",
    expected: "no-counterexample",
  },
  {
    name: "explicit-scopes-only",
    prelude: "sig A {} sig B {}",
    facts: "",
    query: "some A and no B",
    kind: "run",
    scope: "for exactly 1 A, 0 B, 0 Int",
    expected: "instance",
  },
  {
    name: "empty-relation-arity",
    prelude: "sig N { edge: set N }",
    facts: "",
    query: "edge = (none -> none) and no ^(none -> none) and no (none + none)",
    kind: "run",
    scope: "for 1 but 0 Int",
    expected: "instance",
  },
];

type Relation =
  | { readonly kind: "edge" | "all" | "iden" | "none" }
  | {
      readonly kind: "transpose" | "closure" | "reflexive";
      readonly child: Relation;
    }
  | {
      readonly kind: "union" | "intersection" | "difference" | "join";
      readonly left: Relation;
      readonly right: Relation;
    };
type Formula =
  | {
      readonly kind: "no" | "some" | "one" | "lone";
      readonly relation: Relation;
    }
  | {
      readonly kind: "equal" | "in";
      readonly left: Relation;
      readonly right: Relation;
    }
  | { readonly kind: "not"; readonly child: Formula }
  | {
      readonly kind: "and" | "or" | "implies" | "iff";
      readonly left: Formula;
      readonly right: Formula;
    };

const relationSource = (expression: Relation): string => {
  switch (expression.kind) {
    case "edge":
      return "edge";
    case "all":
      return "(Node -> Node)";
    case "iden":
      return "iden";
    case "none":
      return "(none -> none)";
    case "transpose":
    case "closure":
    case "reflexive":
      return `(${expression.kind === "transpose" ? "~" : expression.kind === "closure" ? "^" : "*"}${relationSource(expression.child)})`;
    default:
      return `(${relationSource(expression.left)} ${{ union: "+", intersection: "&", difference: "-", join: "." }[expression.kind]} ${relationSource(expression.right)})`;
  }
};
const formulaSource = (expression: Formula): string => {
  switch (expression.kind) {
    case "no":
    case "some":
    case "one":
    case "lone":
      return `(${expression.kind} ${relationSource(expression.relation)})`;
    case "equal":
    case "in":
      return `(${relationSource(expression.left)} ${expression.kind === "equal" ? "=" : "in"} ${relationSource(expression.right)})`;
    case "not":
      return `(not ${formulaSource(expression.child)})`;
    default:
      return `(${formulaSource(expression.left)} ${expression.kind} ${formulaSource(expression.right)})`;
  }
};
const relationValue = (
  expression: Relation,
  edges: ReadonlySet<number>,
  nodes: number,
): ReadonlySet<number> => {
  const universe = Array.from({ length: nodes * nodes }, (_, index) => index);
  switch (expression.kind) {
    case "edge":
      return edges;
    case "all":
      return new Set(universe);
    case "iden":
      return new Set(
        universe.filter((pair) => Math.floor(pair / nodes) === pair % nodes),
      );
    case "none":
      return new Set();
    case "transpose":
      return new Set(
        [...relationValue(expression.child, edges, nodes)].map(
          (pair) => (pair % nodes) * nodes + Math.floor(pair / nodes),
        ),
      );
    case "closure":
    case "reflexive": {
      const relation = relationValue(expression.child, edges, nodes);
      const result = new Set<number>();
      for (let origin = 0; origin < nodes; origin++) {
        const pending = [origin];
        const visited = new Set<number>();
        if (expression.kind === "reflexive")
          result.add(origin * nodes + origin);
        while (pending.length) {
          const current = pending.pop();
          if (current === undefined) throw new Error("Missing node");
          for (let target = 0; target < nodes; target++)
            if (relation.has(current * nodes + target)) {
              result.add(origin * nodes + target);
              if (!visited.has(target)) {
                visited.add(target);
                pending.push(target);
              }
            }
        }
      }
      return result;
    }
    default: {
      const left = relationValue(expression.left, edges, nodes);
      const right = relationValue(expression.right, edges, nodes);
      if (expression.kind === "union") return new Set([...left, ...right]);
      if (expression.kind === "intersection")
        return new Set([...left].filter((pair) => right.has(pair)));
      if (expression.kind === "difference")
        return new Set([...left].filter((pair) => !right.has(pair)));
      const result = new Set<number>();
      for (const first of left)
        for (const second of right)
          if (first % nodes === Math.floor(second / nodes))
            result.add(Math.floor(first / nodes) * nodes + (second % nodes));
      return result;
    }
  }
};
const formulaValue = (
  expression: Formula,
  edges: ReadonlySet<number>,
  nodes: number,
): boolean => {
  switch (expression.kind) {
    case "no":
    case "some":
    case "one":
    case "lone": {
      const count = relationValue(expression.relation, edges, nodes).size;
      return expression.kind === "no"
        ? count === 0
        : expression.kind === "some"
          ? count > 0
          : expression.kind === "one"
            ? count === 1
            : count <= 1;
    }
    case "equal":
    case "in": {
      const left = relationValue(expression.left, edges, nodes);
      const right = relationValue(expression.right, edges, nodes);
      return (
        [...left].every((pair) => right.has(pair)) &&
        (expression.kind === "in" || left.size === right.size)
      );
    }
    case "not":
      return !formulaValue(expression.child, edges, nodes);
    default: {
      const left = formulaValue(expression.left, edges, nodes);
      const right = formulaValue(expression.right, edges, nodes);
      return expression.kind === "and"
        ? left && right
        : expression.kind === "or"
          ? left || right
          : expression.kind === "implies"
            ? !left || right
            : left === right;
    }
  }
};
export const generated = (seed: number, count: number): readonly Case[] => {
  let state = seed >>> 0;
  const random = (maximum: number) => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) % maximum;
  };
  const relation = (depth: number): Relation => {
    const choice = random(depth ? 11 : 4);
    if (choice < 4)
      return {
        kind:
          choice === 0
            ? "edge"
            : choice === 1
              ? "all"
              : choice === 2
                ? "iden"
                : "none",
      };
    if (choice < 7)
      return {
        kind:
          choice === 4 ? "transpose" : choice === 5 ? "closure" : "reflexive",
        child: relation(depth - 1),
      };
    return {
      kind:
        choice === 7
          ? "union"
          : choice === 8
            ? "intersection"
            : choice === 9
              ? "difference"
              : "join",
      left: relation(depth - 1),
      right: relation(depth - 1),
    };
  };
  const formula = (depth: number): Formula => {
    const choice = random(depth ? 11 : 6);
    if (choice < 4)
      return {
        kind:
          choice === 0
            ? "no"
            : choice === 1
              ? "some"
              : choice === 2
                ? "one"
                : "lone",
        relation: relation(2),
      };
    if (choice < 6)
      return {
        kind: choice === 4 ? "equal" : "in",
        left: relation(2),
        right: relation(2),
      };
    if (choice === 6) return { kind: "not", child: formula(depth - 1) };
    return {
      kind:
        choice === 7
          ? "and"
          : choice === 8
            ? "or"
            : choice === 9
              ? "implies"
              : "iff",
      left: formula(depth - 1),
      right: formula(depth - 1),
    };
  };
  return Array.from({ length: count }, (_, index) => {
    const nodes = 2 + random(2);
    const facts = formula(1);
    const query = formula(2);
    const kind = random(2) ? "check" : "run";
    let feasible = 0;
    let found = false;
    for (let mask = 0; mask < 2 ** (nodes * nodes); mask++) {
      const edges = new Set(
        Array.from({ length: nodes * nodes }, (_, pair) => pair).filter(
          (pair) => mask & (1 << pair),
        ),
      );
      if (!formulaValue(facts, edges, nodes)) continue;
      feasible++;
      if (formulaValue(query, edges, nodes) === (kind === "run")) found = true;
    }
    return {
      name: `generated-${seed.toString(16)}-${index}`,
      prelude: "sig Node { edge: set Node }",
      facts: formulaSource(facts),
      query: formulaSource(query),
      kind,
      scope: `for ${nodes} but exactly ${nodes} Node, 0 Int`,
      expected: found
        ? kind === "run"
          ? "instance"
          : "counterexample"
        : kind === "check" && feasible > 0
          ? "no-counterexample"
          : "no-instance",
    };
  });
};

export const controls = [
  "sig A { r: set A } run { r = none }",
  "sig A { r: set A } run { r = none + none }",
  "sig A {} run { no ^none }",
  "var sig A {} run {}",
  "sig A {} sig B extends A {} run {}",
  "sig A {} sig B in A {} run {}",
  "open util/ordering[A] sig A {} run {}",
  "sig A {} pred P[x:A] {} run {}",
  "sig A {} run { #A = 1 }",
  "sig A {} pred P { P } run {}",
  "sig A {} pred P { unknown in A } run {}",
  "sig A {} fact { A } run {}",
  "sig A {} run { ^A = A }",
  "sig A { r: Missing } run {}",
  "sig A {} run { missing in A }",
  "one sig A {} run {} for 2 but 2 A",
  "sig A {} run {} for 1 but 1 A, 2 A",
  "sig A {} sig B {} run {} for 1 A",
  "some sig A {} run {} for 1 but 0 A",
  "sig A {} run {} for 1 but 6 Int",
] as const;

export const pin = (instance: Instance): string => {
  const names = new Map<string, string>();
  const declarations = Object.entries(instance.signatures).flatMap(
    ([signature, atoms]) =>
      atoms.map((atom, index) => {
        const name = `w_${signature}_${index}`;
        names.set(atom, name);
        return { name, signature };
      }),
  );
  const atomName = (atom: string): string => {
    const name = names.get(atom);
    if (name) return name;
    if (/^-?\d+$/.test(atom)) return atom;
    throw new Error(`Unknown witness atom ${atom}`);
  };
  const constraints = [
    ...Object.entries(instance.signatures).map(
      ([name, atoms]) =>
        `${name} = ${atoms.length ? atoms.map(atomName).join(" + ") : "none"}`,
    ),
    ...Object.entries(instance.fields).map(([name, tuples]) =>
      tuples.length
        ? `${name} = (${tuples.map((tuple) => `(${tuple.map(atomName).join(" -> ")})`).join(" + ")})`
        : `no ${name}`,
    ),
  ];
  const body = constraints.length
    ? constraints.map((constraint) => `(${constraint})`).join(" and ")
    : "no none";
  return declarations.length
    ? `some ${declarations.map((declaration) => `${declaration.name}: ${declaration.signature}`).join(", ")} | ${body} and ${
        declarations
          .flatMap((first, index) =>
            declarations
              .slice(index + 1)
              .filter((second) => second.signature === first.signature)
              .map((second) => `${first.name} != ${second.name}`),
          )
          .join(" and ") || "no none"
      }`
    : body;
};
