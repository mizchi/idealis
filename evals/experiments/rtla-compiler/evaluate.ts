import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { PortableCoreError } from "../../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyProgram } from "../../../packages/idealis/src/safety-program.js";
import { checkSafety } from "../../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../../packages/idealis/src/tla-safety.js";
import {
  checkTlaSource,
  compileTlaSource,
} from "../../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { corpus } from "../rtla-parser/corpus.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { digest } from "../usable/provenance.js";
import { assertOracle } from "../workspace-oracle/node.js";

const directory = process.argv[2];
assert.ok(directory);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const base = await fingerprint();
const sources = [
  ...base.sources,
  ...(await Promise.all(
    [
      "evals/plans/0029-rtla-compiler.md",
      "evals/experiments/rtla-compiler/evaluate.ts",
      "evals/experiments/rtla-parser/corpus.ts",
      "evals/experiments/collections-temporal/cases.ts",
    ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  )),
].sort((left, right) => left.path.localeCompare(right.path));
const sourceVersion = digest(JSON.stringify(sources));
const paths = [
  "crates/idealis-core/target/release/idealis-core",
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
];
const binaries = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const bounds = {
  maxDepth: 6,
  maxStates: 1000,
  maxWork: 100000,
  maxTemporalNodes: 10000,
};
const inputs = await corpus();
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "0029-rtla-compiler",
      sourceVersion,
      sources,
      binaries,
      bounds,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      dirty: execFileSync("git", ["status", "--porcelain"]).toString(),
      normalization:
        "Omit empty optional property/fairness arrays and null ENABLED subscript. Preserve expressions, branch names, branch order and steps.",
      performanceMeasurement: false,
    },
    null,
    2,
  ),
  { flag: "wx" },
);
await writeFile(
  join(directory, "inputs.json"),
  JSON.stringify(inputs, null, 2),
  { flag: "wx" },
);
const core = await createPortableWasm(await readFile(paths[1] ?? ""));
const native = (request: unknown): unknown => {
  const result = spawnSync(paths[0] ?? "", [], {
    input: `${JSON.stringify({ version: 1, request })}\n`,
    encoding: "utf8",
  });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, result.stderr);
  const value: unknown = JSON.parse(result.stdout);
  assert.ok(isRecord(value));
  return value;
};
const normalize = (input: unknown): unknown => {
  if (Array.isArray(input)) return input.map(normalize);
  if (!isRecord(input)) return input;
  return Object.fromEntries(
    Object.entries(input)
      .filter(
        ([key, value]) =>
          !(key === "subscript" && value === null) &&
          !(
            [
              "fairness",
              "initialProperties",
              "actionProperties",
              "properties",
            ].includes(key) &&
            Array.isArray(value) &&
            !value.length
          ),
      )
      .map(([key, value]) => [key, normalize(value)]),
  );
};
const capture = <Value>(
  run: () => Value,
):
  | Value
  | {
      readonly kind: "error";
      readonly code: string;
      readonly message: string;
    } => {
  try {
    return run();
  } catch (error) {
    if (!(error instanceof PortableCoreError)) throw error;
    return { kind: "error", code: error.code, message: error.message };
  }
};
const records: unknown[] = [];
let accepted = 0;
let rejected = 0;
try {
  for (const input of inputs) {
    const reference = (() => {
      try {
        return { program: compileTlaSafety(input) };
      } catch (error) {
        return { diagnostic: String(error) };
      }
    })();
    const wasm = capture(() => compileTlaSource(core, input));
    const compiled = native({
      kind: "compile-tla",
      source: input.source,
      config: input.config,
    });
    assert.ok(isRecord(compiled));
    if (!reference.program) {
      records.push({
        name: input.name,
        status: "rejected",
        reference: reference.diagnostic,
        wasm,
        native: compiled,
      });
      assert.ok(isRecord(wasm) && wasm.kind === "error", input.name);
      assert.equal(compiled.kind, "error", input.name);
      rejected++;
      continue;
    }
    assert.equal(
      compiled.kind,
      "compiled",
      `${input.name}: ${JSON.stringify(compiled)}`,
    );
    assertOracle(normalize(wasm), normalize(reference.program), input.name);
    assertOracle(
      normalize(decodeSafetyProgram(compiled.program)),
      normalize(reference.program),
      input.name,
    );
    const expected = capture(() => ({
      kind: "safety",
      result: checkSafety(core, reference.program, bounds),
    }));
    const checked = capture(() => ({
      kind: "safety",
      result: checkTlaSource(core, input, bounds),
    }));
    const nativeChecked = native({
      kind: "check-tla",
      source: input.source,
      config: input.config,
      bounds,
    });
    records.push({
      name: input.name,
      status: "matched",
      program: wasm,
      expected,
      wasm: checked,
      native: nativeChecked,
    });
    assertOracle(checked, expected, input.name);
    assertOracle(nativeChecked, expected, input.name);
    accepted++;
  }
  for (const source of sources)
    assert.equal(
      digest(await readFile(source.path)),
      source.sha256,
      source.path,
    );
  await writeFile(
    join(directory, "records.json"),
    JSON.stringify(records, null, 2),
    { flag: "wx" },
  );
  const result = {
    status: "complete",
    sourceVersion,
    cases: inputs.length,
    accepted,
    rejected,
    nativeWasm: true,
    sourceChecking: true,
    performanceMeasurement: false,
    files: await Promise.all(
      ["manifest.json", "inputs.json", "records.json"].map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  };
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(result, null, 2),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "rtla-compiler", status: "complete", sourceVersion })}\n`,
  );
  console.log(JSON.stringify({ cases: inputs.length, accepted, rejected }));
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", message: String(error), records },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "rtla-compiler", status: "error", sourceVersion })}\n`,
  );
  throw error;
}
