import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { digest } from "../smt-incremental/provenance.js";
import { counts } from "./cases.js";

// compare <native-dev> <native-conf> <wasm-dev> <wasm-conf> <output>
// Decision fixed in evals/plans/0075-smt-cache-transport.md.
const paths = process.argv.slice(2, 6);
const output = process.argv[6];
assert.ok(output && paths.length === 4);
type Estimate = { name: string; medianNs: number; cacheBytes?: number };
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
      return value;
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

/** Least-squares slope of the median over N, in ns per entry. */
const slope = (values: readonly number[]) => {
  const xs = counts.map(Number);
  const mx = xs.reduce((a, b) => a + b) / xs.length;
  const my = values.reduce((a, b) => a + b) / values.length;
  return (
    xs.reduce((sum, x, index) => sum + (x - mx) * (values[index]! - my), 0) /
    xs.reduce((sum, x) => sum + (x - mx) ** 2, 0)
  );
};
const native = [nativeDev, nativeConf].map((run) => {
  const check = run.median("protocol-check").medianNs;
  const at = (kind: string, count: number) =>
    run.median(`${kind}-${count}`).medianNs;
  return {
    run: run.path,
    checkNs: check,
    rows: counts.map((count) => ({
      entries: count,
      hitNs: at("protocol-hit", count),
      missNs: at("protocol-miss", count),
      storeHitNs: at("store-hit", count),
      storeMissNs: at("store-miss", count),
      hitOverCheck: at("protocol-hit", count) / check,
      missOverCheck: at("protocol-miss", count) / check,
      hitOverStore: at("protocol-hit", count) / at("store-hit", count),
    })),
    hitSlopeNsPerEntry: slope(counts.map((count) => at("protocol-hit", count))),
    missSlopeNsPerEntry: slope(
      counts.map((count) => at("protocol-miss", count)),
    ),
    storeHitSlopeNsPerEntry: slope(
      counts.map((count) => at("store-hit", count)),
    ),
  };
});
const wasm = [wasmDev, wasmConf].map((run) => {
  const check = run.median("wasm-check").medianNs;
  const at = (kind: string, count: number) =>
    run.median(`wasm-${kind}-${count}`);
  return {
    run: run.path,
    checkNs: check,
    rows: counts.map((count) => ({
      entries: count,
      hitNs: at("hit", count).medianNs,
      missNs: at("miss", count).medianNs,
      hitOverCheck: at("hit", count).medianNs / check,
      missOverCheck: at("miss", count).medianNs / check,
      missCacheBytes: at("miss", count).cacheBytes,
    })),
    hitSlopeNsPerEntry: slope(counts.map((count) => at("hit", count).medianNs)),
    missSlopeNsPerEntry: slope(
      counts.map((count) => at("miss", count).medianNs),
    ),
    bytesPerEntry: at("miss", 4096).cacheBytes! / 4096,
  };
});
const slower = (side: typeof native | typeof wasm) =>
  side.every(
    (run) => run.rows.find((row) => row.entries === 512)!.hitOverCheck >= 1,
  );
const result = {
  status: "complete",
  plan: "0075-smt-cache-transport",
  decision: slower(native) || slower(wasm) ? "implement-store" : "hold-store",
  nativeHitSlowerAt512: slower(native),
  wasmHitSlowerAt512: slower(wasm),
  scope: "transport cost of one chain-32 query; no claim about other problems",
  runs: paths,
  native,
  wasm,
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
  `${JSON.stringify({ runId: basename(output), kind: "smt-protocol-comparison", plan: "0075-smt-cache-transport", status: "complete", decision: result.decision })}\n`,
);
console.log(JSON.stringify({ decision: result.decision }));
