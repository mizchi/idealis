import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, loadavg, release } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { digest } from "../smt-subset/provenance.js";

// backends <development|confirmation> <backend_times binary> <run-dir>
// Times both ralloy backends per corpus command (plan 0087).
const [phase, binary, directory] = process.argv.slice(2);
assert.ok(
  binary && directory && (phase === "development" || phase === "confirmation"),
);
const bytes = await readFile(binary);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const save = (path: string, data: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(data, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  plan: "0087-ralloy-backend-threshold",
  phase,
  binary: { path: binary, sha256: digest(bytes) },
  rustc: execFileSync("rustc", ["--version"]).toString().trim(),
  environment: {
    cpu: cpus()[0]?.model,
    os: release(),
    loadAverage: loadavg(),
    dedicatedHost: false,
  },
});
const run = spawnSync(resolve(binary), [], {
  encoding: "utf8",
  maxBuffer: 64 * 1024 * 1024,
});
assert.equal(run.status, 0, run.stderr);
await writeFile(join(directory, "records.jsonl"), run.stdout, { flag: "wx" });
const records = run.stdout
  .trim()
  .split("\n")
  .map((line) => JSON.parse(line));
await save("result.json", {
  status: "complete",
  phase,
  commands: records.length,
  files: [
    { path: "records.jsonl", sha256: digest(run.stdout) },
    {
      path: "manifest.json",
      sha256: digest(await readFile(join(directory, "manifest.json"))),
    },
  ],
});
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(directory), kind: "ralloy-backend-times", plan: "0087-ralloy-backend-threshold", phase, status: "complete" })}\n`,
);
console.log(JSON.stringify({ phase, commands: records.length }));
