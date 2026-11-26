import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import type { SafetyResult } from "../../../packages/idealis/src/contracts.js";
import { decodeSafetyProgram } from "../../../packages/idealis/src/safety-program.js";
import { digest } from "../usable/provenance.js";

interface Observation {
  readonly input: string;
  readonly route: string;
  readonly iteration: number;
  readonly warmup: boolean;
  readonly status: string;
  readonly compileMs: number;
  readonly checkMs: number;
  readonly sourceMs: number;
  readonly irBytes: number;
  readonly definitions: number | null;
  readonly result?: SafetyResult;
  readonly response?: unknown;
  readonly program?: unknown;
  readonly compileBytesSha256?: string;
  readonly checkBytesSha256?: string;
}
interface Cold {
  readonly input: string;
  readonly route: string;
  readonly iteration: number;
  readonly warmup: boolean;
  readonly wallMs: number;
  readonly maxRssBytes: number;
}
interface Results {
  readonly status: string;
  readonly records: readonly Observation[];
  readonly cold: readonly Cold[];
}
interface Manifest {
  readonly inputs: readonly { readonly name: string }[];
  readonly specs: readonly {
    readonly name: string;
    readonly version: number;
  }[];
}

const [input, output] = process.argv.slice(2);
const plan = process.env.IDEALIS_CHECKER_PLAN ?? "0063-evaluator-tuning";
assert.ok(input && output);
const resultBytes = await readFile(join(input, "result.json"));
const result: Results = JSON.parse(resultBytes.toString());
const manifest: Manifest = JSON.parse(
  await readFile(join(input, "manifest.json"), "utf8"),
);
assert.equal(result.status, "complete");
assert.equal(manifest.specs.length, 8);
assert.equal(result.records.length, manifest.inputs.length * 8 * 4);
assert.equal(result.cold.length, (manifest.inputs.length - 4) * 4 * 4);
let comparisons = 0;
let byteComparisons = 0;
let byteRecorded = 0;
const byteDifferences: string[] = [];
for (const model of manifest.inputs) {
  for (const version of [1, 2]) {
    for (const iteration of [0, 1, 2, 3]) {
      const records = await Promise.all(
        [
          `before-native-v${version}`,
          `after-native-v${version}`,
          `before-wasm-v${version}`,
          `after-wasm-v${version}`,
        ].map(async (route) => {
          const record: Observation = JSON.parse(
            await readFile(
              join(input, `${model.name}-${iteration}-${route}.json`),
              "utf8",
            ),
          );
          assert.equal(record.route, route);
          assert.equal(record.input, model.name);
          assert.equal(record.iteration, iteration);
          if (record.status === "complete")
            assert.equal(decodeSafetyProgram(record.program).version, version);
          else assert.equal(record.status, "unsupported");
          return record;
        }),
      );
      const expected = records[0];
      assert.ok(expected);
      for (const record of records)
        if (record.compileBytesSha256 !== undefined) byteRecorded++;
      for (const record of records.slice(1)) {
        assert.equal(record.status, expected.status);
        assert.deepEqual(record.program, expected.program);
        assert.deepEqual(record.result, expected.result);
        assert.deepEqual(record.response, expected.response);
        comparisons++;
        if (
          record.compileBytesSha256 !== undefined &&
          expected.compileBytesSha256 !== undefined
        ) {
          for (const field of [
            "compileBytesSha256",
            "checkBytesSha256",
          ] as const) {
            if (record[field] === undefined && expected[field] === undefined)
              continue;
            byteComparisons++;
            if (record[field] !== expected[field])
              byteDifferences.push(
                `${model.name} v${version} iteration ${iteration} ${expected.route} vs ${record.route} ${field}`,
              );
          }
        }
      }
    }
  }
}
const stats = (values: readonly number[]) => {
  const sorted = values.toSorted((left, right) => left - right);
  assert.equal(sorted.length, 3);
  assert.ok(sorted.every(Number.isFinite));
  return { min: sorted[0], median: sorted[1], max: sorted[2] };
};
const models = manifest.inputs.map((model) => ({
  name: model.name,
  routes: manifest.specs.map((spec) => {
    const records = result.records.filter(
      (record) =>
        record.input === model.name &&
        record.route === spec.name &&
        !record.warmup,
    );
    assert.deepEqual(
      records.map((record) => record.iteration).toSorted(),
      [1, 2, 3],
    );
    const cold = result.cold.filter(
      (record) =>
        record.input === model.name &&
        record.route === spec.name &&
        !record.warmup,
    );
    return {
      name: spec.name,
      status: records[0]?.status,
      irBytes: records[0]?.irBytes,
      compileMs: stats(records.map((record) => record.compileMs)),
      checkMs:
        records[0]?.status === "complete"
          ? stats(records.map((record) => record.checkMs))
          : null,
      sourceMs:
        records[0]?.status === "complete"
          ? stats(records.map((record) => record.sourceMs))
          : null,
      coldMs: cold.length ? stats(cold.map((record) => record.wallMs)) : null,
      rssBytes: cold.length
        ? stats(cold.map((record) => record.maxRssBytes))
        : null,
    };
  }),
}));
await mkdir(output);
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify({ status: "complete", plan, input, resultSha256: digest(resultBytes), comparisons, ...(byteRecorded > 0 ? { byteComparisons, byteDifferences, byteChecks: "Raw response bytes (sha256 of the response text before parsing) of the compile-tla and check-safety requests, compared across before/after native/Wasm for each version." } : {}), checks: "Exact IR version, compiled program, result including work, and unsupported response equality for each version across before/after native/Wasm, including warmups.", models }, null, 2)}\n`,
  { flag: "wx" },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(output), plan, kind: "corpus-summary", status: "complete" })}\n`,
);
if (byteDifferences.length > 0) process.exitCode = 1;
console.log(
  JSON.stringify({
    comparisons,
    ...(byteRecorded > 0
      ? { byteComparisons, byteDifferences: byteDifferences.length }
      : {}),
    model: models.find((model) => model.name === "TCommit-8"),
  }),
);
