import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, loadavg, release } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { benches, isBench } from "./cases.js";
import { digest, fingerprint } from "./provenance.js";

const stage = process.argv[2];
const provenance = process.argv[3];
const directory = process.argv[4];
assert.ok(directory && provenance && stage);
const binary = JSON.parse(await readFile(provenance, "utf8"));
assert.ok(isBench(binary.bench));
const cases = benches[binary.bench as keyof typeof benches];
assert.equal(digest(await readFile(binary.binary)), binary.sha256);
const driver = await fingerprint();
assert.deepEqual(
  driver.sources.filter((file) =>
    file.path.startsWith("crates/rusmt/benches/"),
  ),
  binary.benchmarkSources,
);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const save = (path: string, data: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(data, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  stage,
  plan: "0074-smt-incremental-cache",
  binary,
  driver,
  environment: {
    cpu: cpus()[0]?.model,
    logicalCpus: cpus().length,
    platform: process.platform,
    arch: process.arch,
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
    command: [binary.binary, "--bench", "--noplot"],
    criterionHome: resolve(directory, "criterion"),
    validationOutsideTimers: true,
  },
});
const child = spawn(binary.binary, ["--bench", "--noplot"], {
  env: { ...process.env, CRITERION_HOME: resolve(directory, "criterion") },
  stdio: ["ignore", "pipe", "pipe"],
});
let stdout = "";
let stderr = "";
child.stdout.on("data", (chunk) => {
  stdout += String(chunk);
});
child.stderr.on("data", (chunk) => {
  stderr += String(chunk);
});
const code = await new Promise<number | null>((resolveExit, reject) => {
  child.once("error", reject);
  child.once("close", resolveExit);
});
await writeFile(join(directory, "stdout.log"), stdout, { flag: "wx" });
await writeFile(join(directory, "stderr.log"), stderr, { flag: "wx" });
assert.equal(code, 0, stderr);
const results = [];
for (const name of cases) {
  const root = join("criterion", name, "new");
  const estimates = JSON.parse(
    await readFile(join(directory, root, "estimates.json"), "utf8"),
  );
  const samples = JSON.parse(
    await readFile(join(directory, root, "sample.json"), "utf8"),
  );
  assert.ok(
    Number.isFinite(estimates.median.point_estimate) &&
      estimates.median.point_estimate > 0,
  );
  assert.equal(estimates.median.confidence_interval.confidence_level, 0.95);
  assert.equal(samples.iters.length, 20);
  results.push({
    name,
    medianNs: estimates.median.point_estimate,
    confidence: estimates.median.confidence_interval,
    meanNs: estimates.mean.point_estimate,
  });
}
const paths = [
  "manifest.json",
  "stdout.log",
  "stderr.log",
  ...cases.flatMap((name) =>
    ["estimates.json", "sample.json", "benchmark.json"].map((file) =>
      join("criterion", name, "new", file),
    ),
  ),
];
await save("result.json", {
  status: "complete",
  stage,
  bench: binary.bench,
  results,
  files: await Promise.all(
    paths.map(async (path) => ({
      path,
      sha256: digest(await readFile(join(directory, path))),
    })),
  ),
});
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(directory), kind: "smt-incremental-criterion", plan: "0074-smt-incremental-cache", bench: binary.bench, stage, status: "complete", binary: binary.sha256 })}\n`,
);
console.log(JSON.stringify({ stage, results }));
