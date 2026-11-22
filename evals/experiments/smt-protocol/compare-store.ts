import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { digest } from "../smt-incremental/provenance.js";
import { counts } from "./cases.js";

// compare-store <native-dev> <native-conf> <wasm-dev> <wasm-conf> <0075-comparison> <output>
// Conditions fixed in evals/plans/0076-smt-cache-store.md.
const paths = process.argv.slice(2, 6);
const previous = process.argv[6];
const output = process.argv[7];
assert.ok(output && previous && paths.length === 4);
type Estimate = { name: string; medianNs: number };
const runs = await Promise.all(
  paths.map(async (path) => {
    const read = async (file: string) =>
      JSON.parse(await readFile(join(path, file), "utf8"));
    const manifest = await read("manifest.json");
    const result = await read("result.json");
    assert.equal(result.status, "complete", path);
    for (const file of result.files)
      assert.equal(digest(await readFile(join(path, file.path))), file.sha256);
    const median = (name: string) => {
      const value = (result.results as Estimate[]).find(
        (entry) => entry.name === name,
      );
      assert.ok(value, `${path} ${name}`);
      return value.medianNs;
    };
    return { path, manifest, bench: result.bench as string, median };
  }),
);
const [nativeDev, nativeConf, wasmDev, wasmConf] = runs;
assert.ok(nativeDev && nativeConf && wasmDev && wasmConf);
for (const run of [nativeDev, nativeConf]) assert.equal(run.bench, "protocol");
for (const run of [wasmDev, wasmConf]) assert.equal(run.bench, "wasm-protocol");
assert.equal(
  nativeDev.manifest.binary.sha256,
  nativeConf.manifest.binary.sha256,
);
assert.equal(wasmDev.manifest.wasm.sha256, wasmConf.manifest.wasm.sha256);
const before = JSON.parse(
  await readFile(join(previous, "result.json"), "utf8"),
);
assert.equal(before.plan, "0075-smt-cache-transport");

const side = (
  run: (typeof runs)[number],
  check: string,
  hit: (count: number) => string,
  miss: (count: number) => string,
) => {
  const checkNs = run.median(check);
  const rows = counts.map((count) => ({
    entries: count,
    hitNs: run.median(hit(count)),
    missNs: run.median(miss(count)),
    hitOverCheck: run.median(hit(count)) / checkNs,
    missOverCheck: run.median(miss(count)) / checkNs,
  }));
  const flatness = rows.at(-1)!.hitNs / rows[0]!.hitNs;
  return {
    run: run.path,
    checkNs,
    rows,
    hitFlatness: flatness,
    hitNotSlower: rows.every((row) => row.hitOverCheck <= 1),
    hitIndependentOfN: flatness <= 1.5,
    missBounded: rows.every((row) => row.missOverCheck <= 1.5),
  };
};
const native = [nativeDev, nativeConf].map((run) =>
  side(
    run,
    "protocol-check",
    (count) => `handle-hit-${count}`,
    (count) => `handle-miss-${count}`,
  ),
);
const wasm = [wasmDev, wasmConf].map((run) =>
  side(
    run,
    "wasm-check",
    (count) => `wasm-handle-hit-${count}`,
    (count) => `wasm-handle-miss-${count}`,
  ),
);
const accepted = [...native, ...wasm].every(
  (row) => row.hitNotSlower && row.hitIndependentOfN && row.missBounded,
);
// Report only: the snapshot path against 0075, measured at another time.
const earlier = await Promise.all(
  (before.runs as string[]).map(async (path) => {
    const result = JSON.parse(
      await readFile(join(path, "result.json"), "utf8"),
    );
    return (name: string) =>
      (result.results as Estimate[]).find((entry) => entry.name === name)!
        .medianNs;
  }),
);
const snapshotNames = (prefix: string, check: string) => [
  check,
  ...counts.flatMap((n) => [`${prefix}-hit-${n}`, `${prefix}-miss-${n}`]),
];
const snapshotPath = [
  ...snapshotNames("protocol", "protocol-check").map((name) => ({
    name,
    offset: 0,
  })),
  ...snapshotNames("wasm", "wasm-check").map((name) => ({ name, offset: 2 })),
].map(({ name, offset }) => ({
  name,
  ratiosTo0075: [0, 1].map(
    (index) =>
      runs[offset + index]!.median(name) / earlier[offset + index]!(name),
  ),
}));
const result = {
  status: "complete",
  plan: "0076-smt-cache-store",
  decision: accepted ? "adopt" : "hold",
  scope: "transport cost of one chain-32 query; no claim about other problems",
  runs: paths,
  previous,
  native,
  wasm,
  snapshotPath,
  wasmBytes: (await stat(wasmDev.manifest.wasm.path)).size,
};
await mkdir(dirname(output), { recursive: true });
await mkdir(output);
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify(result, null, 2)}\n`,
  { flag: "wx" },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(output), kind: "smt-protocol-comparison", plan: "0076-smt-cache-store", status: "complete", decision: result.decision })}\n`,
);
console.log(JSON.stringify({ decision: result.decision }));
