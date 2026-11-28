import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Ceiling } from "./wasm-ceiling.js";

const [directory, boundariesPath, output] = process.argv.slice(2);
assert.ok(
  directory && boundariesPath && output,
  "usage: summarize-wasm-0120.js <runs/wasm dir> <boundaries-255.json> <out.md>",
);
const boundaries = JSON.parse(readFileSync(boundariesPath, "utf8")) as Record<
  string,
  { accepted: number }
>;
const expected: Record<string, number> = Object.fromEntries(
  Object.entries(boundaries).map(([kind, value]) => [kind, value.accepted]),
);
expected["hop-source"] = expected.hop ?? 0;
const hosts = [
  ["node default", "node-default"],
  ["node --liftoff-only", "node-liftoff-only"],
  ["node --no-liftoff", "node-no-liftoff"],
  ["chromium main", "chromium:main"],
  ["chromium worker", "chromium:worker"],
] as const;
const load = (side: string, key: string): Ceiling[] => {
  if (key.startsWith("chromium")) {
    const parsed = JSON.parse(
      readFileSync(join(directory, `${side}-chromium.json`), "utf8"),
    ) as Record<string, Ceiling[]>;
    const rows = parsed[key.split(":")[1] ?? ""];
    assert.ok(rows);
    return rows;
  }
  return JSON.parse(
    readFileSync(join(directory, `${side}-${key}.json`), "utf8"),
  ) as Ceiling[];
};
const peakOf = (text: string | null): number | null => {
  const match = /peak=(\d+)/.exec(text ?? "");
  return match?.[1] === undefined ? null : Number(match[1]);
};
const lines = [
  "Oracle-after (limit 4,096, 256 compared with the evaluation depth): largest accepted size per cell and the 256-compared depth it reaches; the minimum over the cells whose next failure is a host failure.",
  "",
  "| host | minimum depth | cell | size | next failure |",
  "| --- | ---: | --- | ---: | --- |",
];
const problems: string[] = [];
const oracleMinimum: Record<string, number> = {};
for (const [label, key] of hosts) {
  const rows = load("oracle-after", key);
  const hostFailures = rows.filter(
    (row) =>
      row.failure !== null &&
      !row.failure.startsWith("error ") &&
      !row.failure.startsWith("bounded"),
  );
  const minimum = hostFailures
    .map((row) => ({ row, depth: peakOf(row.accepted) }))
    .filter(
      (entry): entry is { row: Ceiling; depth: number } => entry.depth !== null,
    )
    .sort((left, right) => left.depth - right.depth)[0];
  assert.ok(minimum, label);
  oracleMinimum[label] = minimum.depth;
  lines.push(
    `| ${label} | ${minimum.depth} | ${minimum.row.cell} | ${minimum.row.largest} | ${(minimum.row.failure ?? "").slice(0, 60)} |`,
  );
  const limited = rows.filter((row) => row.failure?.startsWith("error "));
  for (const row of limited)
    lines.push(
      `|   (${label}: bounded by the 4,096 limit first) | ${peakOf(row.accepted)} | ${row.cell} | ${row.largest} | ${(row.failure ?? "").slice(0, 60)} |`,
    );
  const bounded = rows.filter(
    (row) => row.failure?.startsWith("bounded") || row.largest === null,
  );
  if (bounded.length > 0)
    problems.push(
      `${label}: ${bounded.length} oracle-after cells ended at bounded or no accepted size`,
    );
}
lines.push(
  "",
  "After (limit 256): largest accepted size per cell, compared with the size whose 256-compared depth is 255 (boundaries-255.json).",
  "",
  "| host | cells | largest = expected | typed refusal next | untyped failure |",
  "| --- | ---: | ---: | ---: | ---: |",
);
for (const [label, key] of hosts) {
  const rows = load("after", key);
  let same = 0;
  let typed = 0;
  let untyped = 0;
  for (const row of rows) {
    const want = expected[row.cell];
    if (row.largest === want) same++;
    else
      problems.push(
        `${label} ${row.cell}: largest ${row.largest}, expected ${want}, failure ${(row.failure ?? "").slice(0, 80)}`,
      );
    const failure = row.failure ?? "";
    if (
      failure.startsWith(
        "error protocol: Safety evaluation nesting exceeds 256",
      )
    )
      typed++;
    if (/^(RuntimeError|RangeError)/.test(failure)) untyped++;
  }
  lines.push(`| ${label} | ${rows.length} | ${same} | ${typed} | ${untyped} |`);
}
lines.push("");
const below = Object.entries(oracleMinimum).filter(([, depth]) => depth < 512);
lines.push(
  `oracle-after hosts whose minimum depth is below 512 (required 0): ${below.length}${below.length ? ` (${below.map(([label, depth]) => `${label} ${depth}`).join(", ")})` : ""}`,
);
lines.push(
  `cells whose largest accepted size differs from the expected one, or other problems: ${problems.length}`,
);
for (const problem of problems) lines.push(`- ${problem}`);
writeFileSync(output, `${lines.join("\n")}\n`, { flag: "wx" });
console.log(lines.join("\n"));
