import type { SafetyBounds } from "../../../packages/idealis/src/contracts.js";

export interface BenchmarkCase {
  readonly name: string;
  readonly source: string;
  readonly config: string;
  readonly states: number;
  readonly generated: number;
  readonly initialStates: number;
  readonly layerWidth: number;
  readonly faultPredicate: string;
}
export const bounds: SafetyBounds = {
  maxDepth: 10000000,
  maxStates: 10000000,
  maxWork: 4000000000,
};
const width = (coefficients: readonly number[], dimensions: number) => {
  let layers = [1];
  for (let dimension = 0; dimension < dimensions; dimension++) {
    const next = Array.from(
      { length: layers.length + coefficients.length - 1 },
      () => 0,
    );
    for (const [left, count] of layers.entries())
      for (const [right, factor] of coefficients.entries())
        next[left + right] = (next[left + right] ?? 0) + count * factor;
    layers = next;
  }
  return Math.max(...layers);
};

export const cases = (size: "small" | "large"): readonly BenchmarkCase[] => {
  const radix = size === "large" ? 48 : 3;
  const bits = size === "large" ? 20 : 4;
  const channels = size === "large" ? 7 : 2;
  const ring = size === "large" ? 1000000 : 17;
  const variables = ["a", "b", "c", "d"];
  return [
    {
      name: "counter-product",
      states: radix ** 4,
      generated: 4 * radix ** 4 + 1,
      initialStates: 1,
      layerWidth: width(
        Array.from({ length: radix }, () => 1),
        4,
      ),
      faultPredicate: "a+b+c+d < 4*(M-1)",
      source: `---- MODULE CounterProduct ----\nEXTENDS Naturals\nCONSTANT M\nVARIABLES a,b,c,d\nInit == ${variables.map((name) => `${name}=0`).join(" /\\ ")}\n${variables.map((name) => `Step${name} == ${name}' = (IF ${name}<M-1 THEN ${name}+1 ELSE 0) /\\ UNCHANGED <<${variables.filter((other) => other !== name).join(",")}>>`).join("\n")}\nNext == ${variables.map((name) => `Step${name}`).join(" \\/ ")}\nSafe == ${variables.map((name) => `${name} \\in 0..(M-1)`).join(" /\\ ")}\n====`,
      config: `CONSTANT M=${radix}\nINIT Init\nNEXT Next\nINVARIANT Safe`,
    },
    {
      name: "function-cube",
      states: 2 ** bits,
      generated: bits * 2 ** bits + 1,
      initialStates: 1,
      layerWidth: width([1, 1], bits),
      faultPredicate: "nodes[1]=0",
      source: String.raw`---- MODULE FunctionCube ----
EXTENDS Naturals
CONSTANT N
VARIABLE nodes
Init == nodes = [node \in 1..N |-> 0]
Next == \E node \in 1..N: nodes' = [nodes EXCEPT ![node] = 1-@]
Safe == nodes \in [1..N -> 0..1]
====`,
      config: `CONSTANT N=${bits}\nINIT Init\nNEXT Next\nINVARIANT Safe`,
    },
    {
      name: "parallel-channels",
      states: 8 ** channels,
      generated: 12 * channels * 8 ** (channels - 1) + 1,
      initialStates: 1,
      layerWidth: width([1, 2, 2, 2, 1], channels),
      faultPredicate: String.raw`\A node \in 1..N: chan[node].val=0`,
      source: String.raw`---- MODULE ParallelChannels ----
EXTENDS Naturals
CONSTANT N
VARIABLE chan
Init == chan = [node \in 1..N |-> [val |-> 0, rdy |-> 0, ack |-> 0]]
Send(node,message) == /\ chan[node].rdy=chan[node].ack
                      /\ chan'=[chan EXCEPT ![node].val=message, ![node].rdy=1-@]
Rcv(node) == /\ chan[node].rdy # chan[node].ack
             /\ chan'=[chan EXCEPT ![node].ack=1-@]
Next == \E node \in 1..N: (\E message \in {0,1}: Send(node,message)) \/ Rcv(node)
Safe == chan \in [1..N -> [val:{0,1}, rdy:{0,1}, ack:{0,1}]]
====`,
      config: `CONSTANT N=${channels}\nINIT Init\nNEXT Next\nINVARIANT Safe`,
    },
    {
      name: "linear-ring",
      states: ring,
      generated: ring + 1,
      initialStates: 1,
      layerWidth: 1,
      faultPredicate: "x<N-1",
      source: String.raw`---- MODULE LinearRing ----
EXTENDS Naturals
CONSTANT N
VARIABLE x
Init == x=0
Next == x'=IF x<N-1 THEN x+1 ELSE 0
Safe == x \in 0..(N-1)
====`,
      config: `CONSTANT N=${ring}\nINIT Init\nNEXT Next\nINVARIANT Safe`,
    },
  ];
};

export const fault = (input: BenchmarkCase): BenchmarkCase => {
  const position = input.source.indexOf("Safe ==");
  const predicate = input.source
    .slice(position + "Safe ==".length)
    .replace(/\n====$/, "")
    .trim();
  return {
    ...input,
    source: `${input.source.slice(0, position)}Safe == (${predicate}) /\\ (${input.faultPredicate})\n====`,
  };
};
