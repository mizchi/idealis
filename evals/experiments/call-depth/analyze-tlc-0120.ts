import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import type { PathCell } from "./paths-0120.js";
import type { TlcResult } from "./tlc-0120.js";
import { judge } from "./tlc-judge-0120.js";

const [pathsDirectory, tlcResults, output] = process.argv.slice(2);
assert.ok(
  pathsDirectory && tlcResults && output,
  "usage: analyze-tlc-0120.js <paths dir> <tlc results.json> <out dir>",
);
const sides: Record<
  string,
  Record<string, Record<string, Record<string, PathCell>>>
> = {};
for (const side of ["before", "after", "oracle-after"]) {
  const parsed: unknown = JSON.parse(
    readFileSync(join(pathsDirectory, `${side}.json`), "utf8"),
  );
  assert.ok(isRecord(parsed) && isRecord(parsed.cases));
  sides[side] = parsed.cases as (typeof sides)[string];
}
const tlc = JSON.parse(readFileSync(tlcResults, "utf8")) as Record<
  string,
  TlcResult
>;

const rows: string[] = [];
const mismatches: string[] = [];
const counts: Record<string, Record<string, number>> = {};
const unjudged: string[] = [];
const tally = (side: string, label: string): void => {
  counts[side] ??= {};
  const row = counts[side];
  assert.ok(row);
  row[label] = (row[label] ?? 0) + 1;
};
for (const [name, expected] of Object.entries(tlc)) {
  for (const side of ["before", "after"] as const) {
    for (const path of ["rust-native", "ts-native"] as const) {
      const cell = sides[side]?.[name]?.v2?.[path];
      if (!cell) continue;
      const outcome = cell.outcome;
      const hits = sides["oracle-after"]?.[name]?.v2?.[path]?.probe?.hits256;
      const { label, agrees } = judge(side, expected, cell, {
        oracleAfterHits: hits ?? null,
        before: sides.before?.[name]?.v2?.[path] ?? null,
      });
      tally(`${side}/${path}`, label);
      if (agrees === null)
        unjudged.push(
          `${side} ${path} ${name}: TLC ${expected.verdict}; ${label}`,
        );
      if (agrees === false)
        mismatches.push(
          `${side} ${path} ${name}: ${label}; rtlc ${JSON.stringify(cell.summary)} trace ${JSON.stringify(cell.trace)}; TLC ${expected.verdict} ${expected.states}/${expected.generated} ${JSON.stringify(expected.trace)}`,
        );
      if (side === "after" && path === "rust-native")
        rows.push(
          [
            name,
            expected.verdict,
            expected.states,
            expected.generated,
            JSON.stringify(outcome),
            label,
          ].join("\t"),
        );
    }
  }
}
const lines: string[] = [
  `TLC cases: ${Object.keys(tlc).length}`,
  `mismatches between TLC and rtlc where both reach a verdict: ${mismatches.length}`,
  ...mismatches.map((row) => `  ${row}`),
  `cases whose agreement is not judged (agrees is null; counted, never left out): ${unjudged.length}`,
  ...unjudged.map((row) => `  ${row}`),
  "",
];
const judged = (side: string): number =>
  Object.entries(counts[side] ?? {}).reduce(
    (sum, [label, count]) =>
      label.includes("matches TLC") || label.includes("DIFFERS")
        ? sum + count
        : sum,
    0,
  );
for (const [side, row] of Object.entries(counts)) {
  const total = Object.values(row).reduce((sum, count) => sum + count, 0);
  lines.push(
    `[${side}] cells ${total}, judged against TLC ${judged(side)}, unjudged ${total - judged(side)}`,
  );
  for (const [label, count] of Object.entries(row).sort())
    lines.push(`  ${count} ${label}`);
}
writeFileSync(join(output, "analysis.txt"), `${lines.join("\n")}\n`, {
  flag: "wx",
});
writeFileSync(
  join(output, "verdicts.tsv"),
  `case\ttlc\tstates\tgenerated\tafter rust-native outcome\tclass\n${rows.join("\n")}\n`,
  { flag: "wx" },
);
console.log(lines.join("\n"));
