import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { benches, isBench } from "./cases.js";
import { digest } from "./provenance.js";

// pair <plan> <bench> <baseline> <candidate> <candidate-confirmation> <baseline-confirmation> <output>
// Alternating baseline/candidate pairs for one bench, with thresholds fixed in
// the plan before measuring.
const [plan, bench, ...rest] = process.argv.slice(2);
const output = rest.at(-1);
const paths = rest.slice(0, -1);
assert.ok(output && paths.length === 4 && isBench(bench));
const thresholds: Record<string, (name: string) => number> = {
  // evals/plans/0077-smt-lean-context.md
  "0077-smt-lean-context": (name) =>
    name === "context-assert-256" || name === "context-constant-256"
      ? 0.85
      : name === "parse-solve-4" && bench === "solver"
        ? 1.0
        : 1.1,
};
const required = thresholds[plan!];
assert.ok(required, plan);
type Estimate = {
  name: string;
  medianNs: number;
  confidence: { lower_bound: number; upper_bound: number };
};
const runs = await Promise.all(
  paths.map(async (path) => {
    const read = async (file: string) =>
      JSON.parse(await readFile(join(path, file), "utf8"));
    const manifest = await read("manifest.json");
    const result = await read("result.json");
    assert.equal(result.status, "complete");
    assert.equal(result.bench, bench);
    for (const file of result.files)
      assert.equal(digest(await readFile(join(path, file.path))), file.sha256);
    const estimate = (name: string): Estimate => {
      const value = result.results.find(
        (entry: Estimate) => entry.name === name,
      );
      assert.ok(value, name);
      return value;
    };
    return { manifest, estimate };
  }),
);
const [before, after, afterConfirmation, beforeConfirmation] = runs;
assert.ok(before && after && afterConfirmation && beforeConfirmation);
const hash = (run: typeof before) => run.manifest.binary.sha256;
assert.equal(hash(before), hash(beforeConfirmation));
assert.equal(hash(after), hash(afterConfirmation));
assert.notEqual(hash(before), hash(after));
for (const run of runs)
  assert.deepEqual(
    run.manifest.binary.benchmarkSources,
    before.manifest.binary.benchmarkSources,
  );
const rows = benches[bench].map((name) => {
  const [b, a, ac, bc] = runs.map((run) => run.estimate(name));
  const developmentRatio = a!.medianNs / b!.medianNs;
  const confirmationRatio = ac!.medianNs / bc!.medianNs;
  const requiredRatio = required(name);
  return {
    name,
    baselineNs: b!.medianNs,
    candidateNs: a!.medianNs,
    confirmationCandidateNs: ac!.medianNs,
    confirmationBaselineNs: bc!.medianNs,
    developmentRatio,
    confirmationRatio,
    confidenceIntervals: [b, a, ac, bc].map((value) => value!.confidence),
    requiredRatio,
    accepted:
      developmentRatio <= requiredRatio && confirmationRatio <= requiredRatio,
  };
});
const result = {
  status: "complete",
  plan,
  bench,
  decision: rows.every((row) => row.accepted) ? "adopt" : "hold",
  rejected: rows.filter((row) => !row.accepted).map((row) => row.name),
  scope: "native microbenchmarks; no Z3 or Wasm speed claim",
  runs: paths,
  rows,
};
await mkdir(dirname(output), { recursive: true });
await mkdir(output);
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify(result, null, 2)}\n`,
  { flag: "wx" },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(output), kind: "smt-incremental-comparison", plan, bench, status: "complete", decision: result.decision })}\n`,
);
console.log(
  JSON.stringify({ decision: result.decision, rejected: result.rejected }),
);
