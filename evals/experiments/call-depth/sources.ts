import { readFileSync } from "node:fs";
import type { Source } from "../nesting-chains/sources.js";

export interface DepthSource extends Source {
  readonly depth: number | null;
}

export const depthLimit = 256;
export const hopLevels = [40, 41, 76, 83, 84, 85, 168, 169] as const;
export const unchangedLengths = [6, 7, 8, 9] as const;
export const wrapperConfig =
  "INIT q_init\nNEXT q_step\nINVARIANT q_inv\nCHECK_DEADLOCK FALSE\n";
const invariantConfig =
  "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n";

export const hopDepth = (levels: number): number => 5 + 3 * levels;
export const invariantHopDepth = (levels: number): number => 6 + 3 * levels;

const indices = (count: number): readonly number[] =>
  Array.from({ length: count }, (_, index) => index);

const hops = (levels: number, innermost = "x + 1"): string =>
  [
    `hop0(x) == ${innermost}`,
    ...indices(levels).map(
      (index) => `hop${index + 1}(x) == hop${index}(x) + 1`,
    ),
  ].join("\n");

const wrapped = (declarations: string, step: string, inv: string): string =>
  `---- MODULE CallDepth ----\nEXTENDS Integers\nVARIABLE counter\n${declarations}\ninit == counter = 0\ninv == ${inv}\nstep == ${step}\nq_init == init\nq_step == step\nq_inv == inv\n====\n`;

const recursiveSum =
  "RECURSIVE Sum(_)\nSum(n) == IF n = 0 THEN 0 ELSE n + Sum(n - 1)";

const row = (
  name: string,
  group: string,
  source: string,
  config: string,
  depth: number | null,
): DepthSource => ({
  name,
  group,
  source,
  config,
  equivalent: null,
  semantic: null,
  depth,
});

const walk = "counter' = IF counter < 4 THEN counter + 1 ELSE 0";

const hopSources = (): readonly DepthSource[] =>
  hopLevels.map((levels) =>
    row(
      `hop-${levels}`,
      "hop",
      wrapped(
        hops(levels),
        `counter' = hop${levels}(counter) % 5`,
        "counter >= 0",
      ),
      wrapperConfig,
      hopDepth(levels),
    ),
  );

const invariantHopSources = (): readonly DepthSource[] =>
  hopLevels.map((levels) =>
    row(
      `hop-invariant-${levels}`,
      "hop",
      wrapped(hops(levels), walk, `hop${levels}(counter) >= 0`),
      wrapperConfig,
      invariantHopDepth(levels),
    ),
  );

const chainSources = (): readonly DepthSource[] => [
  row(
    "chain-N50",
    "hop",
    readFileSync("fixtures/call-depth/chain_N50.tla", "utf8"),
    wrapperConfig,
    155,
  ),
];

const semanticSources = (): readonly DepthSource[] => {
  return [
    row(
      "division-by-zero-70",
      "semantic",
      wrapped(
        hops(70, "x \\div 0"),
        `counter' = hop70(counter) % 5`,
        "counter >= 0",
      ),
      wrapperConfig,
      hopDepth(70),
    ),
    row(
      "unevaluated-if-300",
      "semantic",
      wrapped(
        hops(300),
        "counter' = IF counter > 100 THEN hop300(counter) ELSE IF counter < 4 THEN counter + 1 ELSE 0",
        "counter >= 0",
      ),
      wrapperConfig,
      null,
    ),
    row(
      "unevaluated-and-300",
      "semantic",
      wrapped(hops(300), walk, "~(counter > 100 /\\ hop300(counter) > 0)"),
      wrapperConfig,
      null,
    ),
    row(
      "violation-76",
      "semantic",
      wrapped(hops(76), walk, "hop76(counter) < 80"),
      wrapperConfig,
      hopDepth(76),
    ),
  ];
};

const aliasChain = (length: number): string =>
  [
    "V0 == x",
    ...indices(length).map(
      (index) => `V${index + 1} == <<V${index}, V${index}>>`,
    ),
  ].join("\n");

const unchangedDepths: Readonly<Record<number, number | null>> = {
  6: 64,
  7: 128,
  8: 256,
  9: null,
};

const unchangedSources = (): readonly DepthSource[] =>
  unchangedLengths.map((length) =>
    row(
      `unchanged-${length}`,
      "limit",
      `---- MODULE CallDepth ----\nEXTENDS Integers\nVARIABLE x\n${aliasChain(length)}\nInit == x = 0\nNext == UNCHANGED V${length}\nInv == x = 0\n====\n`,
      invariantConfig,
      unchangedDepths[length] ?? null,
    ),
  );

const recursionSources = (): readonly DepthSource[] => [
  row(
    "recursion-loop",
    "recursion",
    "---- MODULE CallDepth ----\nVARIABLE x\nRECURSIVE Loop(_)\nLoop(value) == Loop(value)\nInit == x = Loop(0)\nNext == UNCHANGED x\n====\n",
    "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
    null,
  ),
  row(
    "recursion-sum-200",
    "recursion",
    `---- MODULE CallDepth ----\nEXTENDS Integers\nVARIABLE x\n${recursiveSum}\nInit == x = 0\nNext == UNCHANGED x\nInv == Sum(200) >= 0\n====\n`,
    invariantConfig,
    null,
  ),
  row(
    "recursion-from-hop-50",
    "recursion",
    wrapped(
      `${recursiveSum}\n${hops(50, "x + Sum(x)")}`,
      "counter' = hop50(counter) % 5",
      "counter >= 0",
    ),
    wrapperConfig,
    null,
  ),
];

export const sources0119 = (): readonly DepthSource[] => [
  ...hopSources(),
  ...invariantHopSources(),
  ...chainSources(),
  ...semanticSources(),
  ...unchangedSources(),
  ...recursionSources(),
];
