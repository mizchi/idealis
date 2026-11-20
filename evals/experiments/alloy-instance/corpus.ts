import type {
  AlloyInput,
  AlloyInstance,
  AlloyInstanceCheck,
} from "../../../packages/idealis/src/alloy.js";

export interface Case {
  readonly name: string;
  readonly input: AlloyInput;
  readonly instance: AlloyInstance;
  readonly expected: Pick<AlloyInstanceCheck, "status" | "reason">;
}
export const maxWork = 100_000;
const source = (
  prelude: string,
  facts: string,
  property: string,
  scope: string,
  kind: "run" | "check" = "check",
) =>
  `${prelude}\nfact Assumptions { ${facts} }\n${kind === "check" ? `assert Property { ${property} }\nQuery: check Property` : `Query: run { ${property} }`} ${scope}\nFeasible: run {} ${scope}\n`;

export const graph = (
  name: string,
  size: number,
  mask: number,
  hasFacts = true,
  kind: "run" | "check" = "check",
): Case => {
  const nodes = Array.from(
    { length: size },
    (_, index) => `implementation node ${index}`,
  );
  const edges: [string, string][] = [];
  for (let start = 0; start < size; start++)
    for (let target = 0; target < size; target++) {
      if (mask & (1 << (start * size + target)))
        edges.push([nodes[start] ?? "", nodes[target] ?? ""]);
    }
  const reaches = (
    start: string,
    current: string,
    visited: ReadonlySet<string>,
  ): boolean =>
    edges
      .filter((edge) => edge[0] === current)
      .some(
        (edge) =>
          edge[1] === start ||
          (!visited.has(edge[1]) &&
            reaches(start, edge[1], new Set([...visited, edge[1]]))),
      );
  const loop = edges.some((edge) => edge[0] === edge[1]);
  const cycle = nodes.some((node) => reaches(node, node, new Set([node])));
  return {
    name,
    input: {
      source: source(
        "sig N { edge: set N }",
        hasFacts ? "no iden & edge" : "",
        "no iden & ^edge",
        `for ${size} but exactly ${size} N, 0 Int`,
        kind,
      ),
      command: 0,
      maxWork,
    },
    instance: { signatures: { N: nodes }, fields: { edge: edges } },
    expected:
      hasFacts && loop
        ? { status: "invalid", reason: "facts" }
        : cycle
          ? { status: "mismatch", reason: "property" }
          : { status: "matched", reason: null },
  };
};
export const fixed = (): readonly Case[] => [
  graph("dag", 2, 2),
  graph("two-cycle", 2, 6),
  graph("self-loop-fact", 2, 1),
  graph("self-loop-property", 2, 1, false),
  {
    ...graph("scope", 2, 0),
    input: {
      source: source("sig N { edge: set N }", "", "no edge", "for 1 but 0 Int"),
      command: 0,
      maxWork,
    },
    expected: { status: "invalid", reason: "scope" },
  },
  {
    ...graph("field-one", 2, 0),
    input: {
      source: source(
        "sig N { edge: N }",
        "",
        "no edge",
        "for 2 but exactly 2 N, 0 Int",
      ),
      command: 0,
      maxWork,
    },
    expected: { status: "invalid", reason: "declaration" },
  },
  {
    name: "empty-owner",
    input: {
      source: source(
        "sig N { edge: some N }",
        "",
        "no N",
        "for 0 but 0 Int",
        "run",
      ),
      command: 0,
      maxWork,
    },
    instance: { signatures: { N: [] }, fields: { edge: [] } },
    expected: { status: "matched", reason: null },
  },
  {
    name: "some-sig",
    input: {
      source: source("some sig N {}", "", "no N", "for 1 but 0 Int", "run"),
      command: 0,
      maxWork,
    },
    instance: { signatures: { N: [] }, fields: {} },
    expected: { status: "invalid", reason: "declaration" },
  },
  {
    name: "builtin-int",
    input: {
      source: source(
        "one sig N { value: Int }",
        "",
        "some value",
        "for 0",
        "run",
      ),
      command: 0,
      maxWork,
    },
    instance: {
      signatures: { N: ["actor"] },
      fields: { value: [["actor", "-8"]] },
    },
    expected: { status: "matched", reason: null },
  },
  {
    name: "run-query",
    input: {
      source: source(
        "sig N { edge: set N }",
        "",
        "some edge",
        "for 0 but 0 Int",
        "run",
      ),
      command: 0,
      maxWork,
    },
    instance: { signatures: { N: [] }, fields: { edge: [] } },
    expected: { status: "mismatch", reason: "property" },
  },
];
export const generated = (seed: number, count: number): readonly Case[] => {
  let state = seed >>> 0;
  return Array.from({ length: count }, (_, index) => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    const size = (index % 2) + 2;
    return graph(
      `generated-${seed.toString(16)}-${index}`,
      size,
      (state >>> 0) % (1 << (size * size)),
      index % 3 === 0,
      index % 2 === 0 ? "run" : "check",
    );
  });
};
