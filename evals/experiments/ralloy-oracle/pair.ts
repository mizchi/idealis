import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";

// pair <plan> <baseline> <candidate> <candidate-confirmation> <baseline-confirmation> <output>
// Thresholds are fixed in each plan before measuring.
const [plan, ...rest] = process.argv.slice(2);
const output = rest.at(-1);
const paths = rest.slice(0, -1);
assert.ok(plan && output && paths.length === 4);
const thresholds: Record<string, (name: string) => number> = {
  // evals/plans/0079-ralloy-nary-relations.md
  "0079-ralloy-nary-relations": () => 1.1,
  // evals/plans/0080-ralloy-sig-hierarchy.md
  "0080-ralloy-sig-hierarchy": () => 1.1,
};
const required = thresholds[plan];
assert.ok(required, plan);
type Estimate = {
  name: string;
  medianNs: number;
  confidence: { lower_bound: number; upper_bound: number };
};
const runs = await Promise.all(
  paths.map(async (path) => ({
    manifest: JSON.parse(await readFile(join(path, "manifest.json"), "utf8")),
    result: JSON.parse(await readFile(join(path, "result.json"), "utf8")),
  })),
);
const [before, after, afterConfirmation, beforeConfirmation] = runs;
assert.ok(before && after && afterConfirmation && beforeConfirmation);
const hash = (run: typeof before) => run.manifest.binary.sha256;
assert.equal(hash(before), hash(beforeConfirmation));
assert.equal(hash(after), hash(afterConfirmation));
assert.notEqual(hash(before), hash(after));
for (const run of runs) {
  assert.equal(run.result.status, "complete");
  assert.equal(run.manifest.benchSource, before.manifest.benchSource);
}
const estimate = (run: typeof before, name: string): Estimate => {
  const value = run.result.results.find(
    (entry: Estimate) => entry.name === name,
  );
  assert.ok(value, name);
  return value;
};
const rows = (before.result.results as Estimate[]).map(({ name }) => {
  const [b, a, ac, bc] = runs.map((run) => estimate(run, name));
  const developmentRatio = a!.medianNs / b!.medianNs;
  const confirmationRatio = ac!.medianNs / bc!.medianNs;
  const requiredRatio = required(name);
  return {
    name,
    baselineNs: b!.medianNs,
    candidateNs: a!.medianNs,
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
  decision: rows.every((row) => row.accepted) ? "adopt" : "hold",
  rejected: rows.filter((row) => !row.accepted).map((row) => row.name),
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
  `${JSON.stringify({ runId: basename(output), kind: "ralloy-comparison", plan, status: "complete", decision: result.decision })}\n`,
);
console.log(
  JSON.stringify({ decision: result.decision, rejected: result.rejected }),
);
