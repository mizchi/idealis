import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { cases } from "./cases.js";
import { digest } from "./provenance.js";

const paths = process.argv.slice(2, 6);
const output = process.argv[6];
assert.ok(paths.length === 4 && output);
const runs = await Promise.all(
  paths.map(async (path) => {
    const manifest = JSON.parse(
      await readFile(join(path, "manifest.json"), "utf8"),
    );
    const result = JSON.parse(
      await readFile(join(path, "result.json"), "utf8"),
    );
    assert.equal(result.status, "complete");
    for (const file of result.files)
      assert.equal(digest(await readFile(join(path, file.path))), file.sha256);
    return { path, manifest, result };
  }),
);
const before = runs[0];
const after = runs[1];
const afterConfirmation = runs[2];
const beforeConfirmation = runs[3];
assert.ok(before && after && afterConfirmation && beforeConfirmation);
assert.equal(
  before.manifest.binary.sha256,
  beforeConfirmation.manifest.binary.sha256,
);
assert.equal(
  after.manifest.binary.sha256,
  afterConfirmation.manifest.binary.sha256,
);
assert.notEqual(before.manifest.binary.sha256, after.manifest.binary.sha256);
for (const run of runs)
  assert.deepEqual(
    run.manifest.binary.benchmarkSources,
    before.manifest.binary.benchmarkSources,
  );
const rows = cases.map((name) => {
  const estimates = runs.map((run) => {
    const value = run.result.results.find(
      (result: { name: string }) => result.name === name,
    );
    assert.ok(value);
    return value;
  });
  const developmentRatio = estimates[1].medianNs / estimates[0].medianNs;
  const confirmationRatio = estimates[2].medianNs / estimates[3].medianNs;
  const requiredRatio =
    name === "script-parse-repeat-64"
      ? 0.8
      : name === "script-parse-scopes-32"
        ? 0.9
        : 1.1;
  return {
    name,
    baselineNs: estimates[0].medianNs,
    candidateNs: estimates[1].medianNs,
    confirmationCandidateNs: estimates[2].medianNs,
    confirmationBaselineNs: estimates[3].medianNs,
    developmentRatio,
    confirmationRatio,
    developmentSpeedup: 1 / developmentRatio,
    confirmationSpeedup: 1 / confirmationRatio,
    confidenceIntervals: estimates.map((value) => value.confidence),
    requiredRatio,
    accepted:
      developmentRatio <= requiredRatio && confirmationRatio <= requiredRatio,
  };
});
await mkdir(dirname(output), { recursive: true });
await mkdir(output);
const result = {
  status: "complete",
  decision: rows.every((row) => row.accepted) ? "adopt" : "hold",
  scope: "native microbenchmarks; no Z3 or Wasm speed claim",
  runs: paths,
  rows,
};
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify(result, null, 2)}\n`,
  { flag: "wx" },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(output), kind: "smt-criterion-comparison", status: "complete", decision: result.decision })}\n`,
);
console.log(JSON.stringify(result));
