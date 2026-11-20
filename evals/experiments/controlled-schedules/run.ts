import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";
import { nativePath, wasmPath } from "../testing-core/clients.js";
import { sutPath } from "./clients.js";
import { digest, fingerprint } from "./provenance.js";
import { verify } from "./verify.js";

const loomPath =
  "evals/experiments/controlled-schedules/rust/target/release/examples/loom";
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
  const manifest = JSON.parse(
    await readFile(join(parent, "manifest.json"), "utf8"),
  );
  assert.equal(previous.status, "complete");
  assert.equal(manifest.phase, "development");
  assert.equal(previous.sourceVersion, stamp.sourceVersion);
}
const seeds = Array.from(
  { length: phase === "development" ? 16 : 32 },
  (_, index) => (phase === "development" ? 0x38a1 : 0x38c3) + index,
);
const binaries = await Promise.all(
  [nativePath, wasmPath, sutPath, loomPath].map(async (path) => {
    const bytes = await readFile(path);
    return { path, bytes: bytes.length, sha256: digest(bytes) };
  }),
);
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      ...stamp,
      phase,
      seeds,
      parent,
      binaries,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      dirtyPaths: execFileSync("git", ["status", "--short"]).toString().trim(),
      environment: {
        node: process.version,
        platform: process.platform,
        arch: process.arch,
        rustc: execFileSync("rustc", ["-Vv"]).toString().trim(),
      },
      comparison:
        "Explicit actor order, independent enumeration, TS async generators and gated std threads, two-actor Loom final outcomes",
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
  const loom = JSON.parse(execFileSync(loomPath, { encoding: "utf8" }));
  assert.deepEqual(
    loom.results.find((value: { broken: boolean }) => value.broken)?.outcomes,
    checked.exhaustive.find((value) => value.actors === 2)?.outcomes,
  );
  assert.deepEqual(
    loom.results.find((value: { broken: boolean }) => !value.broken)?.outcomes,
    [2],
  );
  assert.equal(loom.maxPermutations, null);
  assert.equal(loom.maxDuration, null);
  assert.equal(loom.preemptionBound, null);
  assert.equal((await fingerprint()).sourceVersion, stamp.sourceVersion);
  await writeFile(join(directory, "loom.json"), JSON.stringify(loom, null, 2), {
    flag: "wx",
  });
  for (const [path, value] of [
    ["records.json.gz", records],
    ["exchanges.json.gz", checked.exchanges],
    ["sut-exchanges.json.gz", checked.sutExchanges],
  ] as const)
    await writeFile(join(directory, path), gzipSync(JSON.stringify(value)), {
      flag: "wx",
    });
  const files = await Promise.all(
    [
      "manifest.json",
      "records.json.gz",
      "exchanges.json.gz",
      "sut-exchanges.json.gz",
      "loom.json",
    ].map(async (path) => ({
      path,
      sha256: digest(await readFile(join(directory, path))),
    })),
  );
  const result = {
    status: "complete",
    sourceVersion: stamp.sourceVersion,
    seeds: checked.seeds,
    normal: checked.normal,
    detected: checked.detected,
    missed: checked.missed,
    exhaustive: checked.exhaustive,
    exchanges: checked.exchanges.length,
    sutExchanges: checked.sutExchanges.length,
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
    `${JSON.stringify({ runId: basename(directory), kind: "controlled-schedules", status: "complete", sourceVersion: stamp.sourceVersion })}\n`,
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
    `${JSON.stringify({ runId: basename(directory), kind: "controlled-schedules", status: "error", sourceVersion: stamp.sourceVersion })}\n`,
  );
  throw error;
}
