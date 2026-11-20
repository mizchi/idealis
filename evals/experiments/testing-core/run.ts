import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";
import { nativePath, wasmPath } from "./clients.js";
import { digest, fingerprint } from "./provenance.js";
import { verify } from "./verify.js";

const phase = process.argv[2];
const directory = process.argv[3];
const parent = process.argv[4];
assert.ok((phase === "development" || phase === "confirmation") && directory);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const stamp = await fingerprint();
if (phase === "confirmation") {
  assert.ok(parent);
  const previous = JSON.parse(
    await readFile(join(parent, "result.json"), "utf8"),
  );
  assert.equal(previous.status, "complete");
  assert.equal(previous.sourceVersion, stamp.sourceVersion);
}
const base = phase === "development" ? 0x36a1 : 0x36c3;
const seeds = Array.from(
  { length: phase === "development" ? 16 : 32 },
  (_, index) => base + index,
);
const binaries = await Promise.all(
  [nativePath, wasmPath].map(async (path) => ({
    path,
    sha256: digest(await readFile(path)),
    bytes: (await readFile(path)).length,
  })),
);
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      ...stamp,
      phase,
      seeds,
      binaries,
      parent,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      environment: {
        node: process.version,
        platform: process.platform,
        arch: process.arch,
      },
      comparison:
        "native/Wasm effect records, TS generator, existing TS shrinker and stateful counter oracle",
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const records: unknown[] = [];
const start = performance.now();
try {
  const checked = await verify(seeds, (record) => records.push(record));
  assert.equal((await fingerprint()).sourceVersion, stamp.sourceVersion);
  await writeFile(
    join(directory, "records.json.gz"),
    gzipSync(JSON.stringify(records)),
    { flag: "wx" },
  );
  await writeFile(
    join(directory, "exchanges.json.gz"),
    gzipSync(JSON.stringify(checked.exchanges)),
    { flag: "wx" },
  );
  const files = await Promise.all(
    ["manifest.json", "records.json.gz", "exchanges.json.gz"].map(
      async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      }),
    ),
  );
  const result = {
    status: "complete",
    sourceVersion: stamp.sourceVersion,
    seeds: seeds.length,
    normalRuns: seeds.length,
    faultDetected: checked.detected,
    exchanges: checked.exchanges.length,
    elapsedMs: performance.now() - start,
    files,
  };
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(result, null, 2),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "testing-core", status: "complete", sourceVersion: stamp.sourceVersion })}\n`,
  );
  console.log(JSON.stringify(result));
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
    `${JSON.stringify({ runId: basename(directory), kind: "testing-core", status: "error", sourceVersion: stamp.sourceVersion })}\n`,
  );
  throw error;
}
