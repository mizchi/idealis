import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { generated, handCases } from "../smt-scripts/corpus.js";
import { nativePath, wasmPath } from "../smt-subset/paths.js";
import { createWasm } from "../smt-subset/wasm.js";
import { digest, fingerprint } from "./provenance.js";
import { verify } from "./verify.js";

const phase = process.argv[2];
const directory = process.argv[3];
const baselinePath = process.argv[4];
const parent = process.argv[5];
assert.ok(
  directory &&
    baselinePath &&
    (phase === "development" || phase === "confirmation"),
);
const source = await fingerprint();
const binaries = await Promise.all(
  [
    nativePath,
    wasmPath,
    baselinePath,
    join(dirname(baselinePath), "idealis_smt.wasm"),
  ].map(async (path) => ({
    path,
    sha256: digest(await readFile(path)),
  })),
);
if (phase === "confirmation") {
  assert.ok(parent);
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
const seed = phase === "development" ? 0x42a1 : 0x42c3;
const inputs = [
  ...handCases(),
  ...generated(seed, phase === "development" ? 32 : 64),
];
const records: unknown[] = [];
const start = performance.now();
const save = async (path: string, value: unknown) =>
  writeFile(join(directory, path), JSON.stringify(value, null, 2), {
    flag: "wx",
  });
await save("manifest.json", {
  ...source,
  binaries,
  baselineSourceCommit: "d5fb2ff",
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
await writeFile(
  join(directory, "inputs.jsonl"),
  `${inputs.map((input) => JSON.stringify(input)).join("\n")}\n`,
  { flag: "wx" },
);
try {
  const summary = await verify(
    inputs,
    await createWasm(await readFile(wasmPath)),
    (record) => records.push(record),
    baselinePath,
    await createWasm(
      await readFile(join(dirname(baselinePath), "idealis_smt.wasm")),
    ),
  );
  assert.equal((await fingerprint()).sourceVersion, source.sourceVersion);
  await writeFile(
    join(directory, "records.jsonl"),
    `${records.map((record) => JSON.stringify(record)).join("\n")}\n`,
    { flag: "wx" },
  );
  await save("result.json", {
    status: "complete",
    sourceVersion: source.sourceVersion,
    seed,
    summary,
    diagnosticMilliseconds: performance.now() - start,
    files: await Promise.all(
      ["manifest.json", "inputs.jsonl", "records.jsonl"].map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "smt-tuning-correctness", phase, status: "complete", sourceVersion: source.sourceVersion })}\n`,
  );
  console.log(JSON.stringify(summary));
} catch (error) {
  await save("error.json", {
    status: "error",
    message: String(error),
    records,
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "smt-tuning-correctness", phase, status: "error", sourceVersion: source.sourceVersion })}\n`,
  );
  throw error;
}
