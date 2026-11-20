import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import type { SafetyResult } from "../../src/contracts.js";
import { decodeSafetyProgram } from "../../src/safety-program.js";
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
      for (const record of records.slice(1)) {
        assert.equal(record.status, expected.status);
        assert.deepEqual(record.program, expected.program);
        assert.deepEqual(record.result, expected.result);
        assert.deepEqual(record.response, expected.response);
        comparisons++;
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
  `${JSON.stringify({ status: "complete", plan: "0063-evaluator-tuning", input, resultSha256: digest(resultBytes), comparisons, checks: "Exact IR version, compiled program, result including work, and unsupported response equality for each version across before/after native/Wasm, including warmups.", models }, null, 2)}\n`,
  { flag: "wx" },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(output), plan: "0063-evaluator-tuning", kind: "corpus-summary", status: "complete" })}\n`,
);
console.log(
  JSON.stringify({
    comparisons,
    model: models.find((model) => model.name === "TCommit-8"),
  }),
);
