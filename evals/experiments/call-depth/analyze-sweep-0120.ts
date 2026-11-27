import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { families } from "./sources-0120.js";
import type { Cell } from "./sweep-0120.js";

interface Parsed {
  readonly outcome: string;
  readonly peakDepth: number | null;
  readonly peakTotal: number | null;
  readonly hits: number | null;
  readonly published: string | null;
}

const parse = (cell: Cell): Parsed => {
  const match =
    /^(.*?)(?: peak=(\d+) total=(\d+) hits=(\d+) published=(.*))?$/.exec(
      cell.stdout,
    );
  assert.ok(match, cell.cell);
  const outcome = (match[1] ?? "").replace(/^(hop|source|kind) /, "");
  return {
    outcome,
    peakDepth: match[2] === undefined ? null : Number(match[2]),
    peakTotal: match[3] === undefined ? null : Number(match[3]),
    hits: match[4] === undefined ? null : Number(match[4]),
    published: match[5] ?? null,
  };
};

const load = (path: string): Map<string, Parsed> => {
  const parsed = JSON.parse(readFileSync(path, "utf8")) as {
    cells: Cell[];
  };
  return new Map(parsed.cells.map((cell) => [cell.cell, parse(cell)]));
};

const [directory, output] = process.argv.slice(2);
assert.ok(
  directory && output,
  "usage: analyze-sweep-0120.js <sweep dir> <out dir>",
);
const before = load(join(directory, "before.json"));
const oracle = load(join(directory, "oracle.json"));
const after = load(join(directory, "after.json"));
const oracleAfter = load(join(directory, "oracle-after.json"));
const names = [...oracle.keys()];
const lines: string[] = [];
const say = (text: string): void => {
  lines.push(text);
};
const list = (title: string, rows: readonly string[]): void => {
  say(`${title}: ${rows.length}`);
  for (const row of rows.slice(0, 20)) say(`  ${row}`);
};
const get = (from: Map<string, Parsed>, name: string): Parsed => {
  const found = from.get(name);
  assert.ok(found, name);
  return found;
};

say(`points: ${names.length}`);
const outcomeOfPublished = (published: string | null, own: string): string =>
  published === null || published === "own"
    ? own
    : published.startsWith("hit:")
      ? `error protocol: ${published.slice(4)}`
      : `UNSUPPORTED ${published}`;

const pairDifferent: string[] = [];
const totalDifferent: string[] = [];
const order: string[] = [];
const hitsOrder: string[] = [];
const beforeSelf: string[] = [];
const afterSelf: string[] = [];
for (const name of names) {
  const o = get(oracle, name);
  const a = get(oracleAfter, name);
  if (o.outcome !== a.outcome)
    pairDifferent.push(`${name}: ${o.outcome} / ${a.outcome}`);
  if (o.peakTotal !== a.peakTotal)
    totalDifferent.push(`${name}: ${o.peakTotal} / ${a.peakTotal}`);
  if ((a.peakDepth ?? 0) > (a.peakTotal ?? 0))
    order.push(`${name}: ${a.peakDepth} > ${a.peakTotal}`);
  if ((a.hits ?? 0) > (o.hits ?? 0))
    hitsOrder.push(`${name}: ${a.hits} > ${o.hits}`);
  const b = get(before, name);
  if (b.outcome !== outcomeOfPublished(o.published, o.outcome))
    beforeSelf.push(
      `${name}: before ${b.outcome} vs published ${o.published} (own ${o.outcome})`,
    );
  const f = get(after, name);
  if (f.outcome !== outcomeOfPublished(a.published, a.outcome))
    afterSelf.push(
      `${name}: after ${f.outcome} vs published ${a.published} (own ${a.outcome})`,
    );
}
list("oracle vs oracle-after raw outcome differs (required 0)", pairDifferent);
list("oracle vs oracle-after peakTotal differs (required 0)", totalDifferent);
list("oracle-after peakDepth > peakTotal (required 0)", order);
list("oracle-after hits > oracle hits (required 0)", hitsOrder);
list(
  "oracle published256 does not reproduce the before output (required 0)",
  beforeSelf,
);
list(
  "after output differs from the oracle-after published256 (required 0)",
  afterSelf,
);

interface Rule {
  readonly label: string;
  readonly former: (size: number) => number;
  readonly depth: (size: number) => number;
  readonly acceptedBefore: number;
  readonly accepted: number;
  readonly range: readonly [number, number];
  readonly cell: (size: number) => string;
}
const rules: Rule[] = [
  {
    label: "hop",
    former: (n) => 5 + 3 * n,
    depth: (n) => 5 + 2 * n,
    acceptedBefore: 83,
    accepted: 125,
    range: [1, 200],
    cell: (n) => `hop-${n}`,
  },
  {
    label: "hop-invariant",
    former: (n) => 6 + 3 * n,
    depth: (n) => 6 + 2 * n,
    acceptedBefore: 83,
    accepted: 124,
    range: [1, 200],
    cell: (n) => `hop-invariant-${n}`,
  },
  {
    label: "forward",
    former: (s) => 2 * s,
    depth: (s) => s + 1,
    acceptedBefore: 127,
    accepted: 254,
    range: [1, 300],
    cell: (s) => `forward-${s}`,
  },
  {
    label: "capture",
    former: (s) => 2 * s,
    depth: (s) => s + 1,
    acceptedBefore: 127,
    accepted: 254,
    range: [1, 300],
    cell: (s) => `capture-${s}`,
  },
  {
    label: "mixed",
    former: (s) => 2 * s,
    depth: (s) => s + 1,
    acceptedBefore: 127,
    accepted: 254,
    range: [1, 300],
    cell: (s) => `mixed-${s}`,
  },
  ...(
    [
      ["forward-set", 250],
      ["forward-function", 249],
    ] as const
  ).map(([label, accepted]) => {
    const constant = (get(oracle, `${label}-200`).peakDepth ?? 0) - 400;
    return {
      label,
      former: (s: number) => 2 * s + constant,
      depth: (s: number) => s + 1 + constant,
      acceptedBefore: Math.floor((255 - constant) / 2),
      accepted,
      range: [1, 300] as const,
      cell: (s: number) => `${label}-${s}`,
    };
  }),
  ...families.map((family) => ({
    label: family.name,
    former: family.formerDepth,
    depth: family.depth,
    acceptedBefore: family.levels[1],
    accepted: family.levels[2],
    range: [1, 300] as const,
    cell: (n: number) => `${family.name}-${n}`,
  })),
];

const boundaries: Record<string, unknown> = {};
const declaredMismatch: string[] = [];
const boundaryMismatch: string[] = [];
for (const rule of rules) {
  let formerAccepted = 0;
  let accepted = 0;
  let firstHit: number | null = null;
  let firstFormerHit: number | null = null;
  const nonMonotone: string[] = [];
  const unverified: string[] = [];
  for (let size = rule.range[0]; size <= rule.range[1]; size++) {
    const o = get(oracle, rule.cell(size));
    const a = get(oracleAfter, rule.cell(size));
    const structuralFormer = rule.former(size);
    const structuralDepth = rule.depth(size);
    for (const [label, probe, structural] of [
      ["former", o, structuralFormer],
      ["after", a, structuralDepth],
    ] as const) {
      const peak = probe.peakDepth ?? -1;
      const matches =
        structural >= 128
          ? peak === structural
          : peak === structural || peak === structural - 1;
      if (!matches)
        declaredMismatch.push(
          `${rule.label} ${size} ${label}: declared ${structural} recorded ${peak}`,
        );
    }
    if (o.hits === null) unverified.push(`oracle ${size}`);
    else if (o.hits === 0) {
      if (firstFormerHit !== null) nonMonotone.push(`oracle ${size}`);
      else if (unverified.length === 0) formerAccepted = size;
    } else firstFormerHit ??= size;
    if (a.hits === null) unverified.push(`oracle-after ${size}`);
    else if (a.hits === 0) {
      if (firstHit !== null) nonMonotone.push(`oracle-after ${size}`);
      else if (unverified.length === 0) accepted = size;
    } else firstHit ??= size;
  }
  const acceptedPoint = get(oracleAfter, rule.cell(accepted));
  const rejectedPoint = get(oracleAfter, rule.cell(accepted + 1));
  boundaries[rule.label] = {
    acceptedBefore: formerAccepted,
    accepted,
    acceptedPeakDepth: acceptedPoint.peakDepth,
    acceptedPeakTotal: acceptedPoint.peakTotal,
    acceptedOutcome: acceptedPoint.outcome,
    rejected: accepted + 1,
    rejectedPeakDepth: rejectedPoint.peakDepth,
    rejectedOutcome: rejectedPoint.outcome,
    rejectedPublished: rejectedPoint.published,
    nonMonotone,
    unverified,
  };
  if (formerAccepted !== rule.acceptedBefore || accepted !== rule.accepted)
    boundaryMismatch.push(
      `${rule.label}: before ${formerAccepted} (predicted ${rule.acceptedBefore}), after ${accepted} (predicted ${rule.accepted})`,
    );
  if (unverified.length > 0)
    boundaryMismatch.push(
      `${rule.label}: hit count missing at ${unverified.join(", ")}`,
    );
  if (nonMonotone.length > 0)
    boundaryMismatch.push(
      `${rule.label}: not monotone ${nonMonotone.join(", ")}`,
    );
}
list(
  "declared depth mismatches against the probes (128 or more: equal; below 128: equal or one less)",
  declaredMismatch,
);
list("accepted maximum differs from the prediction", boundaryMismatch);

const fixed: readonly (readonly [string, number, number])[] = [
  ["operator", 245, 13],
  ["value", 122, 267],
  ["call", 255, 514],
  ["membership", 243, 25],
  ["binding", 231, 482],
  ["enabled", 239, 259],
  ["bind-run", 231, 259],
  ["lazy", 122, 135],
  ["temporal", 255, 775],
  ["update", 243, 512],
  ["function", 62, 138],
  ["action", 255, 274],
];
const unchanged: string[] = [];
for (const [kind, size, work] of fixed) {
  const p = get(oracleAfter, `${kind}-${size}`);
  const next = get(oracleAfter, `${kind}-${size + 1}`);
  const previous = get(oracleAfter, `${kind}-${size - 1}`);
  const o = get(oracle, `${kind}-${size}`);
  const ok =
    (p.hits ?? -1) === 0 &&
    (next.hits ?? 0) > 0 &&
    (previous.hits ?? -1) === 0 &&
    p.outcome.endsWith(`work=${work}`) &&
    p.peakDepth === p.peakTotal &&
    p.peakDepth === o.peakDepth;
  boundaries[kind] = {
    accepted: size,
    acceptedPeakDepth: p.peakDepth,
    acceptedOutcome: p.outcome,
    rejected: size + 1,
    rejectedPeakDepth: next.peakDepth,
    rejectedOutcome: next.outcome,
    rejectedPublished: next.published,
    oracleAcceptedPeakDepth: o.peakDepth,
  };
  if (!ok)
    unchanged.push(
      `${kind} ${size}: ${p.outcome} peak ${p.peakDepth}/${p.peakTotal}/${o.peakDepth} hits ${previous.hits}/${p.hits}/${next.hits}`,
    );
}
list(
  "unchanged kinds whose boundary is not the plan's (required 0)",
  unchanged,
);

const andForward: string[] = [];
for (const [label, from] of [
  ["oracle", oracle],
  ["oracle-after", oracleAfter],
] as const) {
  const both = get(from, "and-forward-both").peakDepth;
  const left = get(from, "and-forward-left").peakDepth;
  const right = get(from, "and-forward-right").peakDepth;
  say(`and-forward ${label}: both ${both}, left ${left}, right ${right}`);
  if (both !== Math.max(left ?? 0, right ?? 0))
    andForward.push(
      `${label}: both ${both} vs max(left ${left}, right ${right})`,
    );
}
list(
  "and-forward peakDepth is not the larger of the two sides (required 0)",
  andForward,
);

writeFileSync(
  join(output, "boundaries-255.json"),
  `${JSON.stringify(boundaries, null, 1)}\n`,
  { flag: "wx" },
);
writeFileSync(join(output, "sweep-analysis.txt"), `${lines.join("\n")}\n`, {
  flag: "wx",
});
console.log(lines.join("\n"));
