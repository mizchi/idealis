import type { Outcome } from "../nesting-chains/outcome.js";
import {
  type DepthSource,
  hopLevels,
  sources0119,
  wrapperConfig,
} from "./sources.js";

export interface DepthSource0120 extends DepthSource {
  readonly formerDepth: number | null;
  readonly after: Outcome | null;
}

export const forwardedHopLevels = [...hopLevels, 124, 125, 126] as const;
export const acceptedHops = 125;
export const acceptedInvariantHops = 124;

export interface Family {
  readonly name: string;
  readonly levels: readonly [number, number, number, number];
  readonly slope: number;
  readonly intercept: number;
  readonly formerDepth: (levels: number) => number;
  readonly depth: (levels: number) => number;
}

export const families: readonly Family[] = [
  {
    name: "record-hop",
    levels: [10, 124, 248, 249],
    slope: 19,
    intercept: 53,
    formerDepth: (levels) => 7 + 2 * levels,
    depth: (levels) => 7 + levels,
  },
  {
    name: "let-hop",
    levels: [10, 49, 83, 84],
    slope: 20,
    intercept: 32,
    formerDepth: (levels) => 6 + 5 * levels,
    depth: (levels) => 6 + 3 * levels,
  },
  {
    name: "set-hop",
    levels: [10, 124, 249, 250],
    slope: 10,
    intercept: 32,
    formerDepth: (levels) => 6 + 2 * levels,
    depth: (levels) => 6 + levels,
  },
  {
    name: "function-hop",
    levels: [10, 123, 246, 247],
    slope: 10,
    intercept: 37,
    formerDepth: (levels) => 9 + 2 * levels,
    depth: (levels) => 9 + levels,
  },
  {
    name: "primed-hop",
    levels: [10, 124, 248, 249],
    slope: 20,
    intercept: 47,
    formerDepth: (levels) => 7 + 2 * levels,
    depth: (levels) => 7 + levels,
  },
];

const indices = (count: number): readonly number[] =>
  Array.from({ length: count }, (_, index) => index);

const walk = "counter' = IF counter < 4 THEN counter + 1 ELSE 0";
const recursiveSum =
  "RECURSIVE Sum(_)\nSum(n) == IF n = 0 THEN 0 ELSE n + Sum(n - 1)";

const module = (
  variable: string,
  declarations: string,
  init: string,
  step: string,
  inv: string,
): string =>
  `---- MODULE CallDepth ----\nEXTENDS Integers\nVARIABLE ${variable}\n${declarations}\ninit == ${init}\ninv == ${inv}\nstep == ${step}\nq_init == init\nq_step == step\nq_inv == inv\n====\n`;

const chain = (
  name: string,
  parameter: string,
  innermost: string,
  levels: number,
  layer: (level: number) => string,
): string =>
  [
    `${name}0(${parameter}) == ${innermost}`,
    ...indices(levels).map((index) => layer(index + 1)),
  ].join("\n");

const hops = (levels: number, innermost = "x + 1"): string =>
  chain("hop", "x", innermost, levels, (level) => {
    return `hop${level}(x) == hop${level - 1}(x) + 1`;
  });

const familySource = (family: string, levels: number): string => {
  switch (family) {
    case "record-hop":
      return module(
        "state",
        chain(
          "rec",
          "s",
          "[s EXCEPT !.count = IF s.count < 4 THEN s.count + 1 ELSE 0]",
          levels,
          (level) => `rec${level}(s) == rec${level - 1}(s)`,
        ),
        "state = [count |-> 0]",
        `state' = rec${levels}(state)`,
        "state.count >= 0",
      );
    case "let-hop":
      return module(
        "counter",
        chain(
          "lhop",
          "x",
          "x + 1",
          levels,
          (level) =>
            `lhop${level}(x) == LET inner == lhop${level - 1}(x) + 1 IN inner`,
        ),
        "counter = 0",
        walk,
        `lhop${levels}(counter) >= 0`,
      );
    case "set-hop":
      return module(
        "counter",
        chain(
          "shop",
          "S",
          "S",
          levels,
          (level) => `shop${level}(S) == shop${level - 1}(S)`,
        ),
        "counter = 0",
        walk,
        `counter \\in shop${levels}(0..4)`,
      );
    case "function-hop":
      return module(
        "counter",
        chain(
          "fhop",
          "f",
          "f[1]",
          levels,
          (level) => `fhop${level}(f) == fhop${level - 1}(f)`,
        ),
        "counter = 0",
        walk,
        `fhop${levels}([k \\in {1} |-> counter]) >= 0`,
      );
    case "primed-hop":
      return module(
        "counter",
        chain(
          "fwd",
          "x",
          "x",
          levels,
          (level) => `fwd${level}(x) == fwd${level - 1}(x)`,
        ),
        "counter = 0",
        `counter' = (IF counter < 4 THEN counter + 1 ELSE 0) /\\ fwd${levels}(counter)' = fwd${levels}((counter + 0)')`,
        "counter >= 0",
      );
    default:
      throw new Error(`family ${family}`);
  }
};

const complete = (
  states: number,
  generated: number,
  work: number,
): Outcome => ({
  stage: "result",
  status: "complete",
  states,
  generated,
  work,
});

const hopOutcome = (levels: number): Outcome =>
  (levels + 1) % 5 === 0
    ? complete(1, 2, 2 * levels + 8)
    : complete(5, 6, 10 * levels + 32);

const invariantHopOutcome = (levels: number): Outcome =>
  complete(5, 6, 10 * levels + 32);

const row = (
  name: string,
  group: string,
  source: string,
  config: string,
  depth: number | null,
  formerDepth: number | null,
  after: Outcome | null,
): DepthSource0120 => ({
  name,
  group,
  source,
  config,
  equivalent: null,
  semantic: null,
  depth,
  formerDepth,
  after,
});

const hopSource = (levels: number, innermost?: string): string =>
  module(
    "counter",
    hops(levels, innermost),
    "counter = 0",
    `counter' = hop${levels}(counter) % 5`,
    "counter >= 0",
  );

const invariantHopSource = (levels: number): string =>
  module(
    "counter",
    hops(levels),
    "counter = 0",
    walk,
    `hop${levels}(counter) >= 0`,
  );

const refined = (input: DepthSource): DepthSource0120 => {
  const hop = /^hop-(\d+)$/.exec(input.name);
  const invariantHop = /^hop-invariant-(\d+)$/.exec(input.name);
  const former = input.depth;
  if (hop?.[1] !== undefined) {
    const levels = Number(hop[1]);
    return {
      ...input,
      depth: 5 + 2 * levels,
      formerDepth: former,
      after: levels <= acceptedHops ? hopOutcome(levels) : null,
    };
  }
  if (invariantHop?.[1] !== undefined) {
    const levels = Number(invariantHop[1]);
    return {
      ...input,
      depth: 6 + 2 * levels,
      formerDepth: former,
      after:
        levels <= acceptedInvariantHops ? invariantHopOutcome(levels) : null,
    };
  }
  const forwarded: Readonly<Record<string, readonly [number, number]>> = {
    "chain-N50": [105, 155],
    "division-by-zero-70": [145, 215],
    "violation-76": [158, 234],
  };
  const known = forwarded[input.name];
  return {
    ...input,
    depth: known?.[0] ?? input.depth,
    formerDepth: known?.[1] ?? input.depth,
    after: null,
  };
};

const hopInputs = (): readonly DepthSource0120[] =>
  ([124, 125, 126] as const).flatMap((levels) => [
    row(
      `hop-${levels}`,
      "hop",
      hopSource(levels),
      wrapperConfig,
      5 + 2 * levels,
      5 + 3 * levels,
      levels <= acceptedHops ? hopOutcome(levels) : null,
    ),
    row(
      `hop-invariant-${levels}`,
      "hop",
      invariantHopSource(levels),
      wrapperConfig,
      6 + 2 * levels,
      6 + 3 * levels,
      levels <= acceptedInvariantHops ? invariantHopOutcome(levels) : null,
    ),
  ]);

const familyInputs = (): readonly DepthSource0120[] =>
  families.flatMap((family) =>
    family.levels.map((levels, position) =>
      row(
        `${family.name}-${levels}`,
        "family",
        familySource(family.name, levels),
        wrapperConfig,
        family.depth(levels),
        family.formerDepth(levels),
        position < 3
          ? complete(5, 6, family.slope * levels + family.intercept)
          : null,
      ),
    ),
  );

const zeroArgumentChain = (
  name: string,
  innermost: string,
  levels: number,
): string =>
  [
    `${name}0 == ${innermost}`,
    ...indices(levels).map((index) => `${name}${index + 1} == ${name}${index}`),
  ].join("\n");

const andForwardInputs = (): readonly DepthSource0120[] => {
  const declarations = [
    chain(
      "fwd",
      "y",
      "y >= 0",
      100,
      (level) => `fwd${level}(y) == fwd${level - 1}(y)`,
    ),
    zeroArgumentChain("c", "counter >= 0", 198),
  ].join("\n");
  return [
    ["both", "fwd100(counter) /\\ c198"],
    ["left", "fwd100(counter) /\\ counter >= 0"],
    ["right", "counter >= 0 /\\ c198"],
  ].map(([name, invariant]) =>
    row(
      `and-forward-${name}`,
      "and-forward",
      module("counter", declarations, "counter = 0", walk, invariant ?? ""),
      wrapperConfig,
      null,
      null,
      null,
    ),
  );
};

const semanticInputs = (): readonly DepthSource0120[] => [
  row(
    "division-by-zero-120",
    "semantic",
    hopSource(120, "x \\div 0"),
    wrapperConfig,
    5 + 2 * 120,
    5 + 3 * 120,
    { stage: "check", message: "Integer division by zero" },
  ),
  row(
    "violation-120",
    "semantic",
    module("counter", hops(120), "counter = 0", walk, "hop120(counter) < 124"),
    wrapperConfig,
    6 + 2 * 120,
    6 + 3 * 120,
    {
      stage: "result",
      status: "invariant-violation",
      trace: 4,
      work: 8 * 120 + 24,
    },
  ),
];

export const recursionHopLevels = 38;

const recursionInputs = (): readonly DepthSource0120[] => [
  row(
    `recursion-from-hop-${recursionHopLevels}`,
    "recursion",
    module(
      "counter",
      `${recursiveSum}\n${hops(recursionHopLevels, "x + Sum(x)")}`,
      "counter = 0",
      `counter' = hop${recursionHopLevels}(counter) % 5`,
      "counter >= 0",
    ),
    wrapperConfig,
    null,
    null,
    null,
  ),
];

export const sources0120 = (
  overridden: readonly DepthSource[],
): readonly DepthSource0120[] => [
  ...[...sources0119(), ...overridden].map(refined),
  ...hopInputs(),
  ...familyInputs(),
  ...andForwardInputs(),
  ...semanticInputs(),
  ...recursionInputs(),
];
