import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { benches } from "./cases.js";
import { digest } from "./provenance.js";

// compare solver <baseline> <candidate> <candidate-confirmation> <baseline-confirmation> <output>
// compare cache <candidate> <candidate-confirmation> <output>
const mode = process.argv[2];
const paths = process.argv.slice(3, -1);
const output = process.argv.at(-1);
assert.ok(
  output &&
    ((mode === "solver" && paths.length === 4) ||
      (mode === "cache" && paths.length === 2)),
);
type Estimate = {
  name: string;
  medianNs: number;
  confidence: { lower_bound: number; upper_bound: number };
};
const runs = await Promise.all(
  paths.map(async (path) => {
    const manifest = JSON.parse(
      await readFile(join(path, "manifest.json"), "utf8"),
    );
    const result = JSON.parse(
      await readFile(join(path, "result.json"), "utf8"),
    );
    assert.equal(result.status, "complete");
    assert.equal(result.bench, mode);
    for (const file of result.files)
      assert.equal(digest(await readFile(join(path, file.path))), file.sha256);
    const estimate = (name: string): Estimate => {
      const value = result.results.find(
        (entry: Estimate) => entry.name === name,
      );
      assert.ok(value, name);
      return value;
    };
    return { path, manifest, estimate };
  }),
);
for (const run of runs)
  assert.deepEqual(
    run.manifest.binary.benchmarkSources,
    runs[0]!.manifest.binary.benchmarkSources,
  );

// Thresholds fixed in evals/plans/0074-smt-incremental-cache.md.
const required = (name: string) =>
  name === "context-assert-256" || name === "context-constant-256"
    ? 0.5
    : name === "context-scopes-32" || name === "script-parse-repeat-64"
      ? 0.8
      : name === "script-parse-scopes-32"
        ? 0.9
        : 1.1;

let result: Record<string, unknown>;
if (mode === "solver") {
  const [before, after, afterConfirmation, beforeConfirmation] = runs;
  assert.ok(before && after && afterConfirmation && beforeConfirmation);
  const hash = (run: typeof before) => run.manifest.binary.sha256;
  assert.equal(hash(before), hash(beforeConfirmation));
  assert.equal(hash(after), hash(afterConfirmation));
  assert.notEqual(hash(before), hash(after));
  const rows = benches.solver.map((name) => {
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
  result = {
    status: "complete",
    plan: "0074-smt-incremental-cache",
    mode,
    decision: rows.every((row) => row.accepted) ? "adopt" : "hold",
    rejected: rows.filter((row) => !row.accepted).map((row) => row.name),
    scope: "native microbenchmarks; no Z3 or Wasm speed claim",
    runs: paths,
    rows,
  };
} else {
  const [development, confirmation] = runs;
  assert.ok(development && confirmation);
  assert.equal(
    development.manifest.binary.sha256,
    confirmation.manifest.binary.sha256,
  );
  const pairs = [
    ...[4, 32, 128].map(
      (count) => [`prepared-${count}`, `cache-miss-${count}`] as const,
    ),
    ...["repeat-64", "scopes-32", "ite-64"].flatMap((name) => [
      [`script-prepared-${name}`, `script-cache-cold-${name}`] as const,
      [`script-prepared-${name}`, `script-cache-warm-${name}`] as const,
    ]),
  ] as const;
  const rows = pairs.map(([reference, cached]) => {
    const ratios = runs.map(
      (run) => run.estimate(cached).medianNs / run.estimate(reference).medianNs,
    );
    return {
      reference,
      cached,
      referenceNs: runs.map((run) => run.estimate(reference).medianNs),
      cachedNs: runs.map((run) => run.estimate(cached).medianNs),
      developmentRatio: ratios[0],
      confirmationRatio: ratios[1],
    };
  });
  // Reported, not gated: the plan only requires warm not to exceed cold.
  const warmNotSlower = ["repeat-64", "scopes-32", "ite-64"].map((name) => ({
    name,
    holds: runs.every(
      (run) =>
        run.estimate(`script-cache-warm-${name}`).medianNs <=
        run.estimate(`script-cache-cold-${name}`).medianNs,
    ),
  }));
  result = {
    status: "complete",
    plan: "0074-smt-incremental-cache",
    mode,
    decision: warmNotSlower.every((row) => row.holds) ? "report" : "hold",
    scope: "fixed cost of the opt-in cache within one candidate binary",
    runs: paths,
    rows,
    warmNotSlower,
  };
}
await mkdir(dirname(output), { recursive: true });
await mkdir(output);
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify(result, null, 2)}\n`,
  { flag: "wx" },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(output), kind: "smt-incremental-comparison", plan: "0074-smt-incremental-cache", mode, status: "complete", decision: result.decision })}\n`,
);
console.log(JSON.stringify(result));
