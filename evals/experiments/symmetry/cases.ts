import type { TlaSourceInput } from "../../../packages/idealis/src/contracts.js";

export interface Input extends TlaSourceInput {
  readonly name: string;
}

export const bounds = {
  maxDepth: 1000,
  maxStates: 1_000_000,
  maxWork: 1_000_000_000,
};
const configuration = (count: number) =>
  `CONSTANT Nodes={${Array.from({ length: count }, (_, index) => `p${index}`).join(",")}}\nINIT Init\nNEXT Next\nINVARIANT Safe\nSYMMETRY Symm`;

export const counter = (
  count: number,
  maximum: number,
  fault = false,
): Input => ({
  name: `counter-${count}-${maximum}-${fault ? "fault" : "safe"}`,
  source: `---- MODULE Differential ----\nEXTENDS Naturals, TLC\nCONSTANT Nodes\nVARIABLE counters\nSymm == Permutations(Nodes)\nInit == counters = [node \\in Nodes |-> 0]\nNext == \\E node \\in Nodes: counters' = [counters EXCEPT ![node] = IF @ < ${maximum} THEN @ + 1 ELSE 0]\nSafe == ${fault ? `\\A node \\in Nodes: counters[node] < ${maximum}` : `counters \\in [Nodes -> 0..${maximum}]`}\n====`,
  config: configuration(count),
});

export const joint = (count: number): Input => ({
  name: `joint-${count}`,
  source: `---- MODULE Differential ----\nEXTENDS TLC\nCONSTANT Nodes\nVARIABLES first, second\nSymm == Permutations(Nodes)\nInit == /\\ first = [n \\in Nodes |-> FALSE] /\\ second = [n \\in Nodes |-> FALSE]\nNext == \\/ (\\E n \\in Nodes: /\\ first' = [first EXCEPT ![n] = ~@] /\\ UNCHANGED second)\n        \\/ (\\E n \\in Nodes: /\\ second' = [second EXCEPT ![n] = ~@] /\\ UNCHANGED first)\nSafe == /\\ first \\in [Nodes -> BOOLEAN] /\\ second \\in [Nodes -> BOOLEAN]\n====`,
  config: configuration(count),
});

export const plain = (input: Input): Input => ({
  ...input,
  name: `${input.name}-plain`,
  config: input.config.replace("\nSYMMETRY Symm", ""),
});

export const recordNodes = (count: number, fault = false): Input => ({
  name: `record-${count}-${fault ? "fault" : "safe"}`,
  source: `---- MODULE Differential ----
EXTENDS Naturals, TLC
CONSTANT Nodes
VARIABLE local
Symm == Permutations(Nodes)
Init == local = [n \\in Nodes |-> [term |-> 0, ready |-> FALSE]]
Next == \\/ (\\E n \\in Nodes: local' = [local EXCEPT ![n].term = IF @ < 2 THEN @ + 1 ELSE 0])
        \\/ (\\E n \\in Nodes: local' = [local EXCEPT ![n].ready = ~@])
Safe == \\A n \\in Nodes: local[n].term ${fault ? "<" : "<="} 2
====`,
  config: configuration(count),
});

export const nodeSets = (count: number, fault = false): Input => ({
  name: `sets-${count}-${fault ? "fault" : "safe"}`,
  source: `---- MODULE Differential ----
EXTENDS TLC
CONSTANT Nodes
VARIABLES first, second
Symm == Permutations(Nodes)
Init == /\\ first = {} /\\ second = {}
Next == \\/ (\\E n \\in Nodes: /\\ first' = (IF n \\in first THEN first \\ {n} ELSE first \\cup {n}) /\\ UNCHANGED second)
        \\/ (\\E n \\in Nodes: /\\ second' = (IF n \\in second THEN second \\ {n} ELSE second \\cup {n}) /\\ UNCHANGED first)
Safe == /\\ ${fault ? "first = {}" : "first \\subseteq Nodes"} /\\ second \\subseteq Nodes
====`,
  config: configuration(count),
});

export const monotone = (count: number, fault = false): Input => {
  const flags = Array.from({ length: count }, (_, index) => `flag${index}`);
  const variables = ["visible", ...flags];
  return {
    name: `por-${count}-${fault ? "fault" : "safe"}`,
    source: `---- MODULE Differential ----
EXTENDS Naturals
VARIABLES ${variables.join(",")}
Init == visible = 0 ${flags.map((name) => `/\\ ${name} = FALSE`).join(" ")}
Next == ${flags.map((name) => `(/\\ ~${name} /\\ ${name}' = TRUE /\\ UNCHANGED <<${variables.filter((variable) => variable !== name).join(",")}>>)`).join(" \\/ ")} \\/ (visible' = (IF visible < 3 THEN visible + 1 ELSE 0) /\\ UNCHANGED <<${flags.join(",")}>>)
Safe == visible ${fault ? "<" : "<="} 3
====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  };
};

export const partialOrderCorpus = (): readonly Input[] => [
  ...[2, 3].flatMap((count) => [
    monotone(count),
    monotone(count, true),
    {
      ...monotone(count),
      name: `por-${count}-deadlock`,
      source: monotone(count).source.replace(
        "(visible' =",
        "(visible < 3 /\\ visible' =",
      ),
    },
  ]),
  ...(
    [
      ["visible", "Safe == visible <= 3", "Safe == ~flag0"],
      ["cycle", "flag0' = TRUE", "flag0' = ~flag0"],
      ["dependent", "(visible' =", "(~flag0 /\\ visible' ="],
      ["reset", "ELSE 0", "ELSE IF flag0 THEN 0 ELSE 0"],
    ] as const
  ).map(([name, before, after]) => ({
    ...monotone(1),
    name: `por-fallback-${name}`,
    source: monotone(1).source.replace(before, after),
  })),
];

export const corpus = (): readonly Input[] => {
  const input = counter(3, 2);
  const pair = joint(3);
  const global = {
    ...pair,
    name: "joint-global",
    source: pair.source
      .replace("VARIABLES first, second", "VARIABLES first, second, phase")
      .replace("|-> FALSE]\nNext", "|-> FALSE] /\\ phase = FALSE\nNext")
      .replace("UNCHANGED second", "UNCHANGED <<second, phase>>")
      .replace(
        "UNCHANGED first)",
        "UNCHANGED <<first, phase>>)\n        \\/ (/\\ phase' = ~phase /\\ UNCHANGED <<first, second>>)",
      ),
  };
  return [
    ...[2, 3].flatMap((count) => [
      recordNodes(count),
      recordNodes(count, true),
      nodeSets(count),
      nodeSets(count, true),
    ]),
    ...[2, 3, 4].flatMap((count) =>
      [1, 2].flatMap((maximum) => [
        counter(count, maximum),
        counter(count, maximum, true),
      ]),
    ),
    joint(2),
    pair,
    global,
    {
      ...input,
      name: "initial-fault",
      source: input.source.replace(
        "Safe == counters \\in [Nodes -> 0..2]",
        "Safe == \\A node \\in Nodes: counters[node] > 0",
      ),
    },
    {
      ...input,
      name: "empty",
      source: input.source.replace(
        "Init == counters",
        "Init == FALSE /\\ counters",
      ),
    },
    {
      ...input,
      name: "deadlock",
      source: input.source
        .replace(
          "Next == \\E node \\in Nodes: counters'",
          "Next == FALSE /\\ counters'",
        )
        .replace(
          "[counters EXCEPT ![node] = IF @ < 2 THEN @ + 1 ELSE 0]",
          "counters",
        ),
    },
    {
      ...input,
      name: "boolean-functions",
      source: input.source
        .replace("|-> 0]", "|-> FALSE]")
        .replace("IF @ < 2 THEN @ + 1 ELSE 0", "~@")
        .replace("Nodes -> 0..2", "Nodes -> BOOLEAN"),
    },
  ];
};
