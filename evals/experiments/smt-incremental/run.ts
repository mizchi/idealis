import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { generated, handCases } from "../smt-scripts/corpus.js";
import { nativePath, wasmPath } from "../smt-subset/paths.js";
import { sequences } from "./context.js";
import { digest, fingerprint } from "./provenance.js";
import { compactRecords } from "./records.js";
import { verify } from "./verify.js";

// run <development|confirmation> <run-dir> <baseline-dir> [parent-run] [plan]
const [phase, directory, baselineDirectory, parent, planName] =
  process.argv.slice(2);
assert.ok(
  directory &&
    baselineDirectory &&
    (phase === "development" || phase === "confirmation"),
);
// Seeds and baselines fixed in each plan.
const plans = {
  "0074-smt-incremental-cache": {
    seeds: { development: 0x74a1, confirmation: 0x74c3 },
    baselineSourceCommit: "bcf018be1",
  },
  "0077-smt-lean-context": {
    seeds: { development: 0x77a1, confirmation: 0x77c3 },
    baselineSourceCommit: "3dd74cb5b",
  },
} as const;
const plan = (planName || "0074-smt-incremental-cache") as keyof typeof plans;
assert.ok(plan in plans, plan);
const baseline = {
  native: join(baselineDirectory, "rusmt"),
  wasm: join(baselineDirectory, "rusmt.wasm"),
};
const source = await fingerprint();
const binaries = await Promise.all(
  [nativePath, wasmPath, baseline.native, baseline.wasm].map(async (path) => ({
    path,
    sha256: digest(await readFile(path)),
  })),
);
if (phase === "confirmation") {
  assert.ok(parent);
  assert.equal(
    JSON.parse(await readFile(join(parent, "manifest.json"), "utf8")).plan,
    plan,
  );
  const previous = JSON.parse(
    await readFile(join(parent, "manifest.json"), "utf8"),
  );
  const result = JSON.parse(
    await readFile(join(parent, "result.json"), "utf8"),
  );
  assert.equal(result.status, "complete");
  assert.equal(previous.sourceVersion, source.sourceVersion);
  assert.deepEqual(previous.binaries, binaries);
  for (const file of result.files)
    assert.equal(digest(await readFile(join(parent, file.path))), file.sha256);
}
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const seed = plans[plan].seeds[phase];
const count = phase === "development" ? 32 : 64;
const inputs = {
  scripts: [...handCases(), ...generated(seed, count)],
  contexts: sequences(seed, count),
};
const save = async (path: string, value: unknown) =>
  writeFile(join(directory, path), JSON.stringify(value, null, 2), {
    flag: "wx",
  });
await save("manifest.json", {
  ...source,
  plan,
  recordFormat: "0074c-smt-compact-cache-records",
  binaries,
  baselineSourceCommit: plans[plan].baselineSourceCommit,
  phase,
  seed,
  parent,
  commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
  dirtyPaths: execFileSync("git", ["status", "--short"]).toString(),
  versions: {
    node: process.version,
    rust: execFileSync("rustc", ["--version"]).toString().trim(),
  },
});
await save("inputs.json", inputs);
const records: unknown[] = [];
const start = performance.now();
const ledger = (status: string) =>
  appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "smt-incremental-correctness", plan, phase, status, sourceVersion: source.sourceVersion })}\n`,
  );
try {
  const summary = await verify(
    inputs,
    (record) => records.push(record),
    baseline,
  );
  assert.equal((await fingerprint()).sourceVersion, source.sourceVersion);
  await writeFile(
    join(directory, "records.jsonl"),
    `${compactRecords(records)
      .map((record) => JSON.stringify(record))
      .join("\n")}\n`,
    { flag: "wx" },
  );
  await save("result.json", {
    status: "complete",
    sourceVersion: source.sourceVersion,
    seed,
    summary,
    diagnosticMilliseconds: performance.now() - start,
    files: await Promise.all(
      ["manifest.json", "inputs.json", "records.jsonl"].map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  });
  await ledger("complete");
  console.log(JSON.stringify(summary));
} catch (error) {
  await save("error.json", {
    status: "error",
    message: String(error),
    records,
  });
  await ledger("error");
  throw error;
}
