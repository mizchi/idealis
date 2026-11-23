import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  appendFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { cpus, loadavg, release } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { digest } from "../smt-subset/provenance.js";

// bench <plan> <stage> <bench-binary> <library-commit> <run-dir>
// Runs crates/ralloy/benches/oracle.rs once under Criterion (20 samples,
// warmup 1 s, measurement 2 s) and keeps the medians and raw samples.
const [plan, stage, binary, library, directory] = process.argv.slice(2);
assert.ok(plan && stage && binary && library && directory);
const groups = ["core"];
const cases: string[] = [];
for (const group of groups)
  for (const file of (
    await readdir(`crates/ralloy/tests/alloy/${group}`)
  ).sort())
    if (file.endsWith(".als")) cases.push(`${group}-${file.slice(0, -4)}`);
const bytes = await readFile(binary);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const save = (path: string, data: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(data, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  plan,
  stage,
  library,
  binary: { path: binary, sha256: digest(bytes) },
  benchSource: digest(await readFile("crates/ralloy/benches/oracle.rs")),
  environment: {
    cpu: cpus()[0]?.model,
    logicalCpus: cpus().length,
    os: release(),
    loadAverage: loadavg(),
    dedicatedHost: false,
  },
  conditions: {
    samples: 20,
    warmupSeconds: 1,
    measurementSeconds: 2,
    statistic: "median",
    confidence: 0.95,
  },
});
const run = spawnSync(resolve(binary), ["--bench", "--noplot"], {
  env: { ...process.env, CRITERION_HOME: resolve(directory, "criterion") },
  encoding: "utf8",
  maxBuffer: 64 * 1024 * 1024,
});
await writeFile(join(directory, "stdout.log"), run.stdout ?? "", {
  flag: "wx",
});
await writeFile(join(directory, "stderr.log"), run.stderr ?? "", {
  flag: "wx",
});
assert.equal(run.status, 0, run.stderr);
const results = [];
for (const name of cases) {
  const root = join(directory, "criterion", name, "new");
  const estimates = JSON.parse(
    await readFile(join(root, "estimates.json"), "utf8"),
  );
  const samples = JSON.parse(await readFile(join(root, "sample.json"), "utf8"));
  assert.equal(samples.iters.length, 20, name);
  results.push({
    name,
    medianNs: estimates.median.point_estimate,
    confidence: estimates.median.confidence_interval,
  });
}
await save("result.json", { status: "complete", plan, stage, results });
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(directory), kind: "ralloy-criterion", plan, stage, status: "complete", binary: digest(bytes) })}\n`,
);
console.log(JSON.stringify({ stage, cases: results.length }));
