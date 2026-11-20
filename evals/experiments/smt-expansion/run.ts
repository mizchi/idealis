import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { nativePath, wasmPath } from "../smt-subset/paths.js";
import { createWasm } from "../smt-subset/wasm.js";
import { generated, handCases } from "./corpus.js";
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
const seed = phase === "development" ? 0x40a1 : 0x40c3;
const inputs = [
  ...handCases(),
  ...generated(seed, phase === "development" ? 64 : 128),
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
  baselineSourceCommit: "95732ef",
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
try {
  const summary = await verify(
    inputs,
    seed,
    await createWasm(await readFile(wasmPath)),
    (record) => records.push(record),
    baselinePath,
    await createWasm(
      await readFile(join(dirname(baselinePath), "idealis_smt.wasm")),
    ),
  );
  assert.equal((await fingerprint()).sourceVersion, source.sourceVersion);
  await save("records.json", records);
  await save("result.json", {
    status: "complete",
    sourceVersion: source.sourceVersion,
    seed,
    summary,
    diagnosticMilliseconds: performance.now() - start,
    files: await Promise.all(
      ["manifest.json", "inputs.json", "records.json"].map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "smt-expansion", phase, status: "complete", sourceVersion: source.sourceVersion })}\n`,
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
    `${JSON.stringify({ runId: basename(directory), kind: "smt-expansion", phase, status: "error", sourceVersion: source.sourceVersion })}\n`,
  );
  throw error;
}
