import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, loadavg, release } from "node:os";
import { basename, dirname, join } from "node:path";
import { digest, fingerprint } from "../smt-incremental/provenance.js";
import { wasmPath } from "../smt-subset/paths.js";
import { createWasm } from "../smt-subset/wasm.js";
import { closeRequest, freshRequest, wasmCases } from "./cases.js";

// wasm-bench <development|confirmation> <run-dir>
// Conditions fixed in evals/plans/0075-smt-cache-transport.md.
const [stage, directory] = process.argv.slice(2);
assert.ok(directory && (stage === "development" || stage === "confirmation"));
const samples = 20;
const warmupNs = 1_000_000_000n;
const sampleNs = 50_000_000;
const bytes = await readFile(wasmPath);
const source = await fingerprint();
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const save = (path: string, data: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(data, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  stage,
  plan: "0075-smt-cache-transport",
  wasm: { path: wasmPath, sha256: digest(bytes) },
  ...source,
  environment: {
    node: process.version,
    cpu: cpus()[0]?.model,
    logicalCpus: cpus().length,
    platform: process.platform,
    arch: process.arch,
    os: release(),
    loadAverage: loadavg(),
    dedicatedHost: false,
  },
  conditions: {
    samples,
    warmupSeconds: 1,
    minimumSampleMilliseconds: 50,
    statistic: "median",
    client: "evals/experiments/smt-subset/wasm.ts createWasm().request",
  },
});
const client = await createWasm(bytes);
const results = [];
for (const item of wasmCases(client)) {
  if (item.fresh) {
    // One untimed open and close around each single timed request.
    const once = () => {
      const request = freshRequest(client, item);
      const begin = process.hrtime.bigint();
      const reply = client.request(request);
      const ns = Number(process.hrtime.bigint() - begin);
      assert.ok(
        reply.kind === "incremental-solution" && reply.stats.writes === 1,
      );
      closeRequest(client, request);
      return ns;
    };
    for (let index = 0; index < 5; index++) once();
    const raw = Array.from({ length: samples }, () => ({
      iterations: 1,
      ns: once(),
    }));
    const sorted = raw.map((value) => value.ns).sort((a, b) => a - b);
    results.push({
      name: item.name,
      entries: item.entries,
      cacheBytes: item.cacheBytes,
      medianNs: (sorted[(samples >> 1) - 1]! + sorted[samples >> 1]!) / 2,
      minimumNs: sorted[0],
      maximumNs: sorted.at(-1),
      raw,
    });
    continue;
  }
  const run = () => client.request(item.request);
  const start = process.hrtime.bigint();
  let warm = 0;
  while (process.hrtime.bigint() - start < warmupNs) {
    run();
    warm++;
  }
  const perCall = Number(process.hrtime.bigint() - start) / warm;
  const iterations = Math.max(1, Math.ceil(sampleNs / perCall));
  const raw: { iterations: number; ns: number }[] = [];
  for (let sample = 0; sample < samples; sample++) {
    const begin = process.hrtime.bigint();
    for (let index = 0; index < iterations; index++) run();
    raw.push({ iterations, ns: Number(process.hrtime.bigint() - begin) });
  }
  const perIteration = raw
    .map((value) => value.ns / value.iterations)
    .sort((a, b) => a - b);
  const medianNs =
    (perIteration[(samples >> 1) - 1]! + perIteration[samples >> 1]!) / 2;
  results.push({
    name: item.name,
    entries: item.entries,
    cacheBytes: item.cacheBytes,
    medianNs,
    minimumNs: perIteration[0],
    maximumNs: perIteration.at(-1),
    raw,
  });
}
await save("result.json", {
  status: "complete",
  stage,
  bench: "wasm-protocol",
  results,
  files: [
    {
      path: "manifest.json",
      sha256: digest(await readFile(join(directory, "manifest.json"))),
    },
  ],
});
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(directory), kind: "smt-protocol-wasm-bench", plan: "0075-smt-cache-transport", stage, status: "complete", wasm: digest(bytes) })}\n`,
);
console.log(
  JSON.stringify({
    stage,
    results: results.map(({ raw: _raw, ...rest }) => rest),
  }),
);
