import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { type Cell, cellFrom } from "../nesting-chains/stack-harness/cell.js";

const reference = 1 << 20;
const ceiling = 64 * 1024;
const step = 4;

export interface Measurement {
  readonly family: string;
  readonly length: number;
  readonly reference: Cell;
  readonly minimumKibibytes: number | null;
  readonly atMinimum: Cell | null;
  readonly belowMinimum: Cell | null;
  readonly runs: number;
}

export const cellArguments = (family: string, length: number): string[] =>
  family === "hop" ? ["hop", String(length)] : ["kind", family, String(length)];

export const measure = (
  binary: string,
  family: string,
  length: number,
): Measurement => {
  let runs = 0;
  const run = (kibibytes: number): Cell => {
    runs++;
    return cellFrom(
      family,
      length,
      spawnSync(binary, [...cellArguments(family, length), String(kibibytes)], {
        encoding: "utf8",
        timeout: 600_000,
      }),
    );
  };
  const expected = run(reference);
  const accepts = (cell: Cell): boolean =>
    cell.exit === 0 && cell.stdout === expected.stdout;
  const unmeasured = {
    family,
    length,
    reference: expected,
    minimumKibibytes: null,
    atMinimum: null,
    belowMinimum: null,
  };
  if (expected.exit !== 0) return { ...unmeasured, runs };
  const widest = run(ceiling);
  if (!accepts(widest)) return { ...unmeasured, runs };
  let low = 0;
  let high = ceiling / step;
  while (high - low > 1) {
    const middle = Math.floor((low + high) / 2);
    if (accepts(run(middle * step))) high = middle;
    else low = middle;
  }
  return {
    family,
    length,
    reference: expected,
    minimumKibibytes: high * step,
    atMinimum: run(high * step),
    belowMinimum: high > 1 ? run((high - 1) * step) : null,
    runs,
  };
};

const [binary, output, cells] = process.argv.slice(2);
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  assert.ok(
    binary && output && cells,
    "usage: stack.js <harness> <output.json> <family:length,...>",
  );
  await writeFile(output, "", { flag: "wx" });
  const measurements: Measurement[] = [];
  for (const cell of cells.split(",")) {
    const [family, length] = cell.split(":");
    assert.ok(family && length, `cell ${cell}`);
    const measurement = measure(binary, family, Number(length));
    measurements.push(measurement);
    console.log(
      cell,
      measurement.reference.stdout ||
        measurement.reference.stderr.slice(0, 120),
      measurement.minimumKibibytes,
      measurement.belowMinimum?.exit ?? measurement.belowMinimum?.signal ?? "-",
    );
  }
  await writeFile(output, `${JSON.stringify(measurements, null, 1)}\n`);
}
