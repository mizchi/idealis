import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { nativePath, wasmPath } from "./clients.js";
import { generated, handCases } from "./corpus.js";
import { digest, fingerprint } from "./provenance.js";
import { verify } from "./verify.js";
import { createWasm } from "./wasm.js";

const phase = process.argv[2];
const directory = process.argv[3];
const parent = process.argv[4];
assert.ok(directory && (phase === "development" || phase === "confirmation"));
const source = await fingerprint();
const binaries = await Promise.all(
  [nativePath, wasmPath].map(async (path) => ({
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
const seed = phase === "development" ? 0x30a1 : 0x30c3;
const inputs = [
  ...handCases(),
  ...generated(seed, phase === "development" ? 64 : 160),
];
const records: unknown[] = [];
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      ...source,
      binaries,
      phase,
      seed,
      parent,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      sourcePathsExcludeDocumentation: true,
    },
    null,
    2,
  ),
  { flag: "wx" },
);
await writeFile(
  join(directory, "inputs.json"),
  JSON.stringify(inputs, null, 2),
  { flag: "wx" },
);
try {
  const summary = await verify(
    inputs,
    await createWasm(await readFile(wasmPath)),
    (record) => records.push(record),
  );
  assert.equal((await fingerprint()).sourceVersion, source.sourceVersion);
  await writeFile(
    join(directory, "records.json"),
    JSON.stringify(records, null, 2),
    { flag: "wx" },
  );
  const result = {
    status: "complete",
    sourceVersion: source.sourceVersion,
    seed,
    ...summary,
    files: await Promise.all(
      ["manifest.json", "inputs.json", "records.json"].map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  };
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(result, null, 2),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "smt-subset-correctness", phase, status: "complete", sourceVersion: source.sourceVersion })}\n`,
  );
  console.log(JSON.stringify(summary));
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", message: String(error), records },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "smt-subset-correctness", phase, status: "error", sourceVersion: source.sourceVersion })}\n`,
  );
  throw error;
}
