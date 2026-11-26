import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { checked, type Outcome } from "../nesting-chains/outcome.js";
import {
  type NamedCase,
  type PathCell,
  type Probe,
  pathNames,
} from "./paths-0120.js";

type PathName = (typeof pathNames)[number];
type Sides = Record<string, Record<string, Record<string, PathCell>>>;

const frontend = (path: PathName): "ts" | "rust" =>
  path.startsWith("ts") ? "ts" : "rust";

const readSide = async (directory: string, side: string): Promise<Sides> => {
  const parsed: unknown = JSON.parse(
    await readFile(join(directory, `${side}.json`), "utf8"),
  );
  assert.ok(isRecord(parsed) && isRecord(parsed.cases));
  return parsed.cases as Sides;
};

interface Expected {
  readonly expect: Record<string, Record<string, Outcome>>;
  readonly depth?: number | null;
  readonly formerDepth?: number | null;
}

const readExpectations = async (
  files: readonly string[],
): Promise<Map<string, Expected>> => {
  const map = new Map<string, Expected>();
  for (const file of files) {
    const parsed: unknown = JSON.parse(await readFile(file, "utf8"));
    assert.ok(Array.isArray(parsed));
    for (const entry of parsed as (NamedCase & Expected)[])
      map.set(entry.name, {
        expect: entry.expect,
        depth: entry.depth ?? null,
        formerDepth: entry.formerDepth ?? null,
      });
  }
  return map;
};

const same = (left: unknown, right: unknown): boolean =>
  JSON.stringify(left) === JSON.stringify(right);

const outcomeOfPublished = (published: unknown): Outcome | null => {
  if (!isRecord(published)) return null;
  return checked(published);
};

const codeOfPublished = (published: unknown): string | null =>
  isRecord(published) && published.kind === "error"
    ? String(published.code)
    : null;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (isRecord(value))
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
      .join(",")}}`;
  return JSON.stringify(value) ?? "null";
};

const canonicalDigest = (value: unknown): string =>
  createHash("sha256").update(canonical(value)).digest("hex");

const [directory, expectFiles, outputDirectory] = process.argv.slice(2);
assert.ok(
  directory && expectFiles && outputDirectory,
  "usage: analyze-0120.js <paths dir> <cases.json,...in increasing priority> <output dir>",
);
const side = {
  before: await readSide(directory, "before"),
  oracle: await readSide(directory, "oracle"),
  after: await readSide(directory, "after"),
  oracleAfter: await readSide(directory, "oracle-after"),
};
const expectations = await readExpectations(expectFiles.split(","));
const names = Object.keys(side.after);
const lines: string[] = [];
const differences: string[] = [];
const say = (text: string): void => {
  lines.push(text);
};
const cell = (
  from: Sides,
  name: string,
  version: "v1" | "v2",
  path: PathName,
): PathCell => {
  const found = from[name]?.[version]?.[path];
  assert.ok(found, `${name} ${version} ${path}`);
  return found;
};
const everyCell = function* (): Generator<{
  name: string;
  version: "v1" | "v2";
  path: PathName;
}> {
  for (const name of names)
    for (const version of ["v1", "v2"] as const)
      for (const path of pathNames) yield { name, version, path };
};
const list = (title: string, rows: readonly string[]): void => {
  say(`${title}: ${rows.length}`);
  for (const row of rows.slice(0, 30)) say(`  ${row}`);
  if (rows.length > 30) say(`  ... ${rows.length - 30} more`);
};

say(`cells: ${names.length * 8} (${names.length} inputs x v1/v2 x 4 paths)`);

const irDifferent: string[] = [];
const wasmNative: Record<string, string[]> = {
  before: [],
  oracle: [],
  after: [],
  oracleAfter: [],
};
for (const { name, version, path } of everyCell()) {
  if (
    cell(side.before, name, version, path).ir !==
    cell(side.after, name, version, path).ir
  )
    irDifferent.push(`${name} ${version} ${path}`);
}
for (const [label, sides] of Object.entries(side))
  for (const name of names)
    for (const version of ["v1", "v2"] as const)
      for (const [wasm, native] of [
        ["ts-wasm", "ts-native"],
        ["rust-wasm", "rust-native"],
      ] as const)
        if (
          !same(
            cell(sides, name, version, wasm).outcome,
            cell(sides, name, version, native).outcome,
          )
        )
          wasmNative[label]?.push(`${name} ${version} ${wasm}/${native}`);
list("before vs after IR sha256 differs", irDifferent);
for (const [label, rows] of Object.entries(wasmNative))
  list(`[${label}] same frontend, Wasm vs native outcome differs`, rows);

const tsRust = { equal: 0, differ: 0, differing: [] as string[] };
for (const label of ["before", "after"] as const)
  for (const name of names)
    for (const version of ["v1", "v2"] as const) {
      const left = cell(side[label], name, version, "ts-native").irNormalized;
      const right = cell(
        side[label],
        name,
        version,
        "rust-native",
      ).irNormalized;
      if (left && right) {
        if (left === right) tsRust.equal++;
        else {
          tsRust.differ++;
          tsRust.differing.push(`${label} ${name} ${version}`);
        }
      }
    }
say(
  `TS vs Rust normalized IR where both compile: equal ${tsRust.equal}, differ ${tsRust.differ}`,
);
for (const row of tsRust.differing.slice(0, 10)) say(`  ${row}`);

const v1Different: string[] = [];
for (const { name, path } of everyCell())
  for (const label of ["oracle", "oracleAfter"] as const) {
    if (
      cell(side.before, name, "v1", path).raw !==
      cell(side[label], name, "v1", path).raw
    )
      v1Different.push(`${label} ${name} ${path}`);
  }
for (const { name, path } of everyCell())
  if (
    cell(side.before, name, "v1", path).raw !==
    cell(side.after, name, "v1", path).raw
  )
    v1Different.push(`after ${name} ${path}`);
list(
  "v1 raw response differs from before (after, oracle, oracle-after)",
  v1Different,
);

let zeroHit = 0;
let someHit = 0;
let noProbe = 0;
const zeroHitDifferent: string[] = [];
const zeroHitOracleDifferent: string[] = [];
const selfCheckFailed: string[] = [];
const selfCheckChecked = { own: 0, hit: 0 };
const afterOracleFailed: string[] = [];
const afterOracleChecked = { own: 0, hit: 0 };
const oraclePair: string[] = [];
const peakTotalDifferent: string[] = [];
const peakOrder: string[] = [];
const hitOrder: string[] = [];
const forwardingInvolved: string[] = [];
const forwardingFreeMismatch: string[] = [];
const expectationMismatch: string[] = [];
const foldDiagnostics: string[] = [];
const changed: Record<string, number> = {};
const bounded: string[] = [];
for (const name of names)
  for (const path of pathNames) {
    const before = cell(side.before, name, "v2", path);
    const oracle = cell(side.oracle, name, "v2", path);
    const after = cell(side.after, name, "v2", path);
    const oracleAfter = cell(side.oracleAfter, name, "v2", path);
    const label = `${name} ${path}`;
    if (oracle.probe === null || oracleAfter.probe === null) {
      noProbe++;
      if (before.raw !== after.raw)
        zeroHitDifferent.push(`${label} (no check reply)`);
      continue;
    }
    const o: Probe = oracle.probe;
    const oa: Probe = oracleAfter.probe;
    if (o.hits256 === 0) {
      zeroHit++;
      if (before.raw !== after.raw) zeroHitDifferent.push(`${label}`);
      if (before.raw !== oracle.raw) zeroHitOracleDifferent.push(label);
    } else someHit++;
    if (o.published256 === null) {
      selfCheckChecked.own++;
      if (before.raw !== oracle.raw) selfCheckFailed.push(`${label} own`);
    } else {
      selfCheckChecked.hit++;
      if (
        !same(before.outcome, outcomeOfPublished(o.published256)) ||
        before.code !== codeOfPublished(o.published256)
      )
        selfCheckFailed.push(
          `${label} published ${JSON.stringify(o.published256)} vs ${JSON.stringify(before.outcome)}`,
        );
    }
    if (oracle.raw !== oracleAfter.raw)
      oraclePair.push(
        `${label} ${JSON.stringify(oracle.outcome)} / ${JSON.stringify(oracleAfter.outcome)}`,
      );
    if (o.peakTotal !== oa.peakTotal)
      peakTotalDifferent.push(`${label} ${o.peakTotal} / ${oa.peakTotal}`);
    if (oa.peakDepth > oa.peakTotal)
      peakOrder.push(`${label} ${oa.peakDepth} > ${oa.peakTotal}`);
    if (oa.hits256 > o.hits256)
      hitOrder.push(`${label} ${oa.hits256} > ${o.hits256}`);
    if (oa.peakDepth !== oa.peakTotal) forwardingInvolved.push(label);
    else if (oa.hits256 !== o.hits256 || o.peakDepth !== oa.peakDepth)
      forwardingFreeMismatch.push(
        `${label} peak ${o.peakDepth}/${oa.peakDepth} hits ${o.hits256}/${oa.hits256}`,
      );
    if (oa.published256 === null) {
      afterOracleChecked.own++;
      if (after.raw !== oracleAfter.raw) afterOracleFailed.push(`${label} own`);
    } else {
      afterOracleChecked.hit++;
      if (
        !same(after.outcome, outcomeOfPublished(oa.published256)) ||
        after.code !== codeOfPublished(oa.published256)
      )
        afterOracleFailed.push(
          `${label} published ${JSON.stringify(oa.published256)} vs ${JSON.stringify(after.outcome)}`,
        );
    }
    if (
      isRecord(o.published256) &&
      String(o.published256.message).startsWith("Unsupported TLA input")
    )
      foldDiagnostics.push(label);
    if (before.raw !== after.raw) {
      const kind = `${before.outcome.stage === "check" ? before.outcome.message : before.outcome.stage === "result" ? before.outcome.status : "compile"} -> ${after.outcome.stage === "check" ? after.outcome.message : after.outcome.stage === "result" ? after.outcome.status : "compile"}`;
      changed[kind] = (changed[kind] ?? 0) + 1;
      differences.push(
        [
          name,
          path,
          JSON.stringify(before.outcome),
          JSON.stringify(after.outcome),
          JSON.stringify(oracle.outcome),
          JSON.stringify(oracleAfter.outcome),
          `hits ${o.hits256}/${oa.hits256}`,
          o.hits256 === 0
            ? "OUTSIDE-ALLOWED"
            : oa.published256 === null
              ? "oracle-after-own"
              : "published256",
        ].join("\t"),
      );
    }
    if (after.outcome.stage === "result" && after.outcome.status === "bounded")
      bounded.push(label);
  }
say(
  `v2 cells with a check reply: hits256 = 0 in ${zeroHit}, hits256 >= 1 in ${someHit}; no check reply (compile refusal) ${noProbe}`,
);
list(
  "hit 0 cells where after differs from before (required 0)",
  zeroHitDifferent,
);
list(
  "hit 0 cells where the oracle differs from before (required 0)",
  zeroHitOracleDifferent,
);
say(
  `oracle published256 self-check: own ${selfCheckChecked.own}, published ${selfCheckChecked.hit}`,
);
list(
  "oracle published256 does not reproduce the before response",
  selfCheckFailed,
);
say(
  `oracle-after published256 vs after: own ${afterOracleChecked.own}, published ${afterOracleChecked.hit}`,
);
list("after differs from oracle-after published256", afterOracleFailed);
list("oracle vs oracle-after raw response differs (required 0)", oraclePair);
list(
  "oracle vs oracle-after peakTotal differs (required 0)",
  peakTotalDifferent,
);
list("oracle-after peakDepth > peakTotal (required 0)", peakOrder);
list("oracle-after hits256 > oracle hits256 (required 0)", hitOrder);
say(
  `cells where oracle-after peakDepth < peakTotal (forwarding involved): ${forwardingInvolved.length}`,
);
list(
  "cells with peakDepth = peakTotal but peak or hits differ from the oracle",
  forwardingFreeMismatch,
);
list("oracle published256 is a fold diagnostic", foldDiagnostics);
say("v2 cells where after differs from before, by kind:");
for (const [kind, count] of Object.entries(changed).sort())
  say(`  ${count} ${kind}`);
list("after bounded cells", bounded);

for (const { name, version, path } of everyCell()) {
  const found = expectations.get(name);
  if (!found) continue;
  const expected = found.expect[frontend(path)]?.[version];
  const actual = cell(side.after, name, version, path).outcome;
  if (expected && !same(actual, expected)) {
    const comparable =
      expected.stage === "result" &&
      actual.stage === "result" &&
      expected.work === null
        ? { ...actual, work: null }
        : actual;
    if (!same(comparable, expected))
      expectationMismatch.push(
        `${name} ${version} ${path}: ${JSON.stringify(actual)} vs ${JSON.stringify(expected)}`,
      );
  }
}
list("after vs recorded expectations mismatches", expectationMismatch);

const derivation: string[] = [];
const derivationBranches = { hitZero: 0, ownOfOracle: 0, published: 0 };
for (const [name, found] of expectations) {
  if (!side.after[name] || found.depth === undefined) continue;
  for (const path of pathNames) {
    const expected = found.expect[frontend(path)]?.v2;
    if (!expected) continue;
    const before = cell(side.before, name, "v2", path);
    const oracle = cell(side.oracle, name, "v2", path);
    const oracleAfter = cell(side.oracleAfter, name, "v2", path);
    let derived: Outcome;
    if (oracle.probe === null) derived = before.outcome;
    else if (oracle.probe.hits256 === 0) {
      derivationBranches.hitZero++;
      derived = before.outcome;
    } else if (oracleAfter.probe !== null && oracleAfter.probe.hits256 === 0) {
      derivationBranches.ownOfOracle++;
      derived = oracle.outcome;
    } else {
      derivationBranches.published++;
      derived =
        outcomeOfPublished(oracleAfter.probe?.published256) ?? oracle.outcome;
    }
    const comparable =
      expected.stage === "result" &&
      derived.stage === "result" &&
      expected.work === null
        ? { ...derived, work: null }
        : derived;
    if (!same(comparable, expected))
      derivation.push(
        `${name} v2 ${path}: derived from the oracles ${JSON.stringify(derived)} vs fixture ${JSON.stringify(expected)}`,
      );
  }
}
say(
  `fixture expectations derived from the oracles: hit 0 ${derivationBranches.hitZero}, oracle own ${derivationBranches.ownOfOracle}, oracle-after published256 ${derivationBranches.published}`,
);
list("fixture expectation differs from the oracle-derived value", derivation);

const declared: string[] = [];
const declaredMismatch: string[] = [];
for (const [name, found] of expectations) {
  for (const [label, probes, value] of [
    ["formerDepth", side.oracle, found.formerDepth],
    ["depth", side.oracleAfter, found.depth],
  ] as const) {
    if (value === null || value === undefined) continue;
    const probe = side.after[name]
      ? cell(probes, name, "v2", "rust-native").probe
      : null;
    if (!probe) continue;
    const recorded = probe.peakDepth;
    const matches =
      value >= 128
        ? recorded === value
        : recorded === value || recorded === value - 1;
    declared.push(
      `${name} ${label} declared ${value} recorded ${recorded} ${matches ? "ok" : "MISMATCH"}`,
    );
    if (!matches)
      declaredMismatch.push(
        `${name} ${label} declared ${value} recorded ${recorded}`,
      );
  }
}
say(`declared depths compared with the probes: ${declared.length}`);
list(
  "declared depth mismatches (128 or more: equal; below 128: equal or one less)",
  declaredMismatch,
);

const bytes = { compared: 0, wire: 0, compileText: 0, unjudged: 0 };
const bytesUnjudged: string[] = [];
const bytesDiffer = (
  label: string,
  left: PathCell,
  right: PathCell,
): string | null => {
  if (left.wire !== null && right.wire !== null) {
    bytes.compared++;
    bytes.wire++;
    return left.wire === right.wire ? null : `${label} response bytes`;
  }
  if (left.compileText !== null && right.compileText !== null) {
    bytes.compared++;
    bytes.compileText++;
    return left.compileText === right.compileText
      ? null
      : `${label} compile error text`;
  }
  bytes.unjudged++;
  bytesUnjudged.push(label);
  return null;
};
const byteDifferent: Record<string, string[]> = {
  "v1 before vs after": [],
  "v1 before vs oracle": [],
  "v1 before vs oracle-after": [],
  "v2 hit 0 before vs after": [],
  "v2 hit 0 before vs oracle": [],
  "v2 oracle vs oracle-after": [],
};
const byteFail: string[] = [];
const publishedBytes = { equal: 0, keyOrderOnly: 0, differ: 0 };
for (const { name, version, path } of everyCell()) {
  const label = `${name} ${version} ${path}`;
  const before = cell(side.before, name, version, path);
  const push = (key: string, found: string | null): void => {
    if (found !== null) byteDifferent[key]?.push(found);
  };
  if (version === "v1") {
    push(
      "v1 before vs after",
      bytesDiffer(label, before, cell(side.after, name, version, path)),
    );
    push(
      "v1 before vs oracle",
      bytesDiffer(label, before, cell(side.oracle, name, version, path)),
    );
    push(
      "v1 before vs oracle-after",
      bytesDiffer(label, before, cell(side.oracleAfter, name, version, path)),
    );
    continue;
  }
  const oracle = cell(side.oracle, name, version, path);
  const oracleAfter = cell(side.oracleAfter, name, version, path);
  const after = cell(side.after, name, version, path);
  if (oracle.probe === null || oracleAfter.probe === null) {
    push("v2 hit 0 before vs after", bytesDiffer(label, before, after));
    continue;
  }
  if (oracle.probe.hits256 === 0) {
    push("v2 hit 0 before vs after", bytesDiffer(label, before, after));
    push("v2 hit 0 before vs oracle", bytesDiffer(label, before, oracle));
  }
  push("v2 oracle vs oracle-after", bytesDiffer(label, oracle, oracleAfter));
  for (const [kind, observed, probe, published] of [
    ["oracle", before, oracle.probe, oracle],
    ["oracle-after", after, oracleAfter.probe, oracleAfter],
  ] as const) {
    if (probe.published256 === null) {
      const found = bytesDiffer(`${label} ${kind} own`, observed, published);
      if (found !== null) byteFail.push(found);
    } else if (published.publishedWire === null || observed.wire === null) {
      bytes.unjudged++;
      bytesUnjudged.push(`${label} ${kind} published256`);
    } else {
      bytes.compared++;
      bytes.wire++;
      if (published.publishedWire === observed.wire) publishedBytes.equal++;
      else if (
        canonicalDigest(probe.published256) === observed.raw &&
        observed.raw !== null
      ) {
        publishedBytes.keyOrderOnly++;
        byteFail.push(`${label} ${kind} published256 differs in key order`);
      } else {
        publishedBytes.differ++;
        byteFail.push(`${label} ${kind} published256 differs beyond key order`);
      }
    }
  }
}
say(
  `byte comparisons (response bytes before parsing, compile error text where there is no response): compared ${bytes.compared} (response ${bytes.wire}, compile text ${bytes.compileText}), unjudged ${bytes.unjudged} (the dump has no wire digest or compile text)`,
);
for (const [key, rows] of Object.entries(byteDifferent))
  list(`bytes: ${key} differs`, rows);
say(
  `published256 branch (bytes of the published reply against the real response): byte-identical ${publishedBytes.equal}, identical only up to object key order ${publishedBytes.keyOrderOnly} (a failure), different beyond key order ${publishedBytes.differ} (a failure)`,
);
const programBytes = {
  compared: 0,
  unjudged: 0,
  differ: [] as string[],
};
for (const { name, version, path } of everyCell()) {
  const before = cell(side.before, name, version, path);
  for (const [label, other] of [
    ["after", cell(side.after, name, version, path)],
    ["oracle", cell(side.oracle, name, version, path)],
    ["oracle-after", cell(side.oracleAfter, name, version, path)],
  ] as const) {
    if (before.programWire === null || other.programWire === null) {
      if (before.compileText === null && other.compileText === null)
        programBytes.unjudged++;
      continue;
    }
    programBytes.compared++;
    if (before.programWire !== other.programWire)
      programBytes.differ.push(`${label} ${name} ${version} ${path}`);
  }
}
say(
  `compile response program bytes (raw text of the program member, before parsing; TS paths hash the JSON text of the program object): compared ${programBytes.compared} (before against after, oracle and oracle-after), unjudged ${programBytes.unjudged}`,
);
list("bytes: program member differs from before", programBytes.differ);
list("bytes: own or published256 comparison fails", byteFail);
list("bytes: cells not judged by bytes", bytesUnjudged);

await writeFile(
  join(outputDirectory, "analysis.txt"),
  `${lines.join("\n")}\n`,
  { flag: "wx" },
);
await writeFile(
  join(outputDirectory, "differences.tsv"),
  `case\tpath\tbefore\tafter\toracle\toracle-after\thits\tclass\n${differences.join("\n")}\n`,
  { flag: "wx" },
);
await writeFile(
  join(outputDirectory, "declared-depths.txt"),
  `${declared.join("\n")}\n`,
  { flag: "wx" },
);
console.log(lines.join("\n"));
