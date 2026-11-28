import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { hopSource } from "./wasm-ceiling.js";

const request = (levels: number): string =>
  JSON.stringify({
    version: 1,
    request: {
      kind: "check-tla",
      source: hopSource(levels),
      config: "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
      bounds: { maxDepth: 64, maxStates: 10000, maxWork: 100_000_000 },
      irVersion: 2,
    },
  });

const [mode, probe, wasm, work, output, ...rest] = process.argv.slice(2);
assert.ok(
  mode && probe && wasm && work && output,
  "usage: go-0120.js check|top <go-probe> <idealis_core.wasm> <work dir> <out.json> [levels... | low high]",
);
mkdirSync(work, { recursive: true });
const run = (levels: readonly number[]): string[] => {
  const files = levels.map((count) => {
    const path = join(work, `hop-${String(count).padStart(4, "0")}.json`);
    writeFileSync(path, request(count), { flag: "w" });
    return path;
  });
  const result = spawnSync(probe, [wasm, ...files], {
    encoding: "utf8",
    timeout: 120_000,
    maxBuffer: 1 << 26,
  });
  assert.equal(
    result.status,
    0,
    `go probe exit ${result.status}: ${result.stderr}`,
  );
  return result.stdout.trim().split("\n");
};

if (mode === "check") {
  const levels = rest.map(Number);
  const lines = run(levels);
  const expected = (count: number): string =>
    count > 125
      ? "error protocol: Safety evaluation nesting exceeds 256"
      : (count + 1) % 5 === 0
        ? `complete 1 2 ${2 * count + 7}`
        : `complete 5 6 ${10 * count + 27}`;
  const rows = levels.map((count, index) => {
    const line = lines[index] ?? "";
    const prefix = `hop-${String(count).padStart(4, "0")}.json `;
    const outcome = line.startsWith(prefix) ? line.slice(prefix.length) : line;
    return {
      levels: count,
      line,
      outcome,
      expected: expected(count),
      matches: outcome === expected(count),
      trap: line.includes("trap:"),
    };
  });
  writeFileSync(output, `${JSON.stringify(rows, null, 1)}\n`, { flag: "wx" });
  for (const row of rows)
    console.log(row.line, row.matches ? "ok" : `EXPECTED ${row.expected}`);
} else {
  assert.equal(mode, "top");
  let [low, high] = rest.map(Number) as [number, number];
  const accepted = (levels: number): string => {
    const line = run([levels])[0] ?? "";
    return line;
  };
  const outcomes = new Map<number, string>();
  const outcome = (levels: number): string => {
    const known = outcomes.get(levels);
    if (known !== undefined) return known;
    const fresh = accepted(levels);
    outcomes.set(levels, fresh);
    return fresh;
  };
  const completes = (line: string): boolean =>
    / complete \d+ \d+ \d+$/.test(line);
  assert.ok(completes(outcome(low)), `low ${low}: ${outcome(low)}`);
  if (completes(outcome(high))) low = high;
  else
    while (high - low > 1) {
      const middle = (low + high) >> 1;
      if (completes(outcome(middle))) low = middle;
      else high = middle;
    }
  const result = {
    largestComplete: low,
    largestLine: outcome(low),
    firstFailing: completes(outcome(high)) ? null : high,
    failingLine: completes(outcome(high)) ? null : outcome(high),
  };
  writeFileSync(output, `${JSON.stringify(result, null, 1)}\n`, { flag: "wx" });
  for (const [levels, line] of outcomes)
    console.log(`probe ${levels}: ${line}`);
  console.log(result.largestLine);
  console.log(result.failingLine);
}
