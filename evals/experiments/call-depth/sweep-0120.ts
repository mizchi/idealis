import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const indices = (count: number): readonly number[] =>
  Array.from({ length: count }, (_, index) => index);

const walk = "counter' = IF counter < 4 THEN counter + 1 ELSE 0";

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

const hops = (levels: number): string =>
  chain("hop", "x", "x + 1", levels, (level) => {
    return `hop${level}(x) == hop${level - 1}(x) + 1`;
  });

export const invariantHopSource = (levels: number): string =>
  module(
    "counter",
    hops(levels),
    "counter = 0",
    walk,
    `hop${levels}(counter) >= 0`,
  );

export const familySource = (family: string, levels: number): string => {
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

export interface Cell {
  readonly cell: string;
  readonly arguments: readonly string[];
  readonly exit: number | null;
  readonly signal: string | null;
  readonly stdout: string;
  readonly stderr: string;
  readonly milliseconds: number;
}

const stackKibibytes = "1048576";

const cases = (path: string): Map<string, { source: string }> => {
  const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
  assert.ok(Array.isArray(parsed));
  return new Map(
    (parsed as { name: string; source: string }[]).map((entry) => [
      entry.name,
      entry,
    ]),
  );
};

const [harness, output, work, fixture, ...specs] = process.argv.slice(2);
assert.ok(
  harness && output && work && fixture && specs.length > 0,
  "usage: sweep-0120.js <harness> <out.json> <work dir> <cases-0120.json> <spec>...",
);
mkdirSync(work, { recursive: true });
const fixtureCases = cases(fixture);
const checkedAgainstFixture: string[] = [];
const sourceFile = (name: string, source: string): string => {
  const path = join(work, `${name}.tla`);
  writeFileSync(path, source, { flag: "w" });
  return path;
};
const runCell = (cell: string, args: readonly string[]): Cell => {
  const started = Date.now();
  const result = spawnSync(harness, [...args], {
    encoding: "utf8",
    timeout: 120_000,
    maxBuffer: 1 << 26,
  });
  return {
    cell,
    arguments: args,
    exit: result.status,
    signal: result.signal,
    stdout: result.stdout.trim(),
    stderr: result.stderr.slice(0, 200),
    milliseconds: Date.now() - started,
  };
};

const cellsOut: Cell[] = [];
for (const spec of specs) {
  const [family, first, low, high] = spec.split(":");
  if (family === "and-forward") {
    for (const kind of ["both", "left", "right"]) {
      const name = `and-forward-${kind}`;
      const entry = fixtureCases.get(name);
      assert.ok(entry, name);
      cellsOut.push(
        runCell(name, [
          "source",
          sourceFile(name, entry.source),
          stackKibibytes,
        ]),
      );
    }
    continue;
  }
  assert.ok(first !== undefined && low !== undefined, `spec ${spec}`);
  if (family === "hop" || family === "hop-invariant" || family === "family") {
    const [start, end] =
      family === "family"
        ? [Number(low), Number(high)]
        : [Number(first), Number(low)];
    const label = family === "family" ? first : family;
    for (let size = start; size <= end; size++) {
      const name = `${label}-${size}`;
      if (family === "hop") {
        cellsOut.push(runCell(name, ["hop", String(size), stackKibibytes]));
        continue;
      }
      const source: string =
        family === "hop-invariant"
          ? invariantHopSource(size)
          : familySource(first, size);
      const known = fixtureCases.get(name);
      if (known) {
        assert.equal(known.source, source, `generator spelling ${name}`);
        checkedAgainstFixture.push(name);
      }
      cellsOut.push(
        runCell(name, ["source", sourceFile(name, source), stackKibibytes]),
      );
    }
    continue;
  }
  assert.equal(family, "kind", `spec ${spec}`);
  assert.ok(low !== undefined && high !== undefined, `spec ${spec}`);
  for (let size = Number(low); size <= Number(high); size++)
    cellsOut.push(
      runCell(`${first}-${size}`, [
        "kind",
        first,
        String(size),
        stackKibibytes,
      ]),
    );
}
writeFileSync(
  output,
  `${JSON.stringify({ harness: "harness", checkedAgainstFixture, cells: cellsOut }, null, 1)}\n`,
  { flag: "wx" },
);
console.log(
  `wrote ${output}: ${cellsOut.length} cells, ${checkedAgainstFixture.length} sources equal to the fixture`,
);
