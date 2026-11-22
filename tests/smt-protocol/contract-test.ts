import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { wasmCases } from "../../evals/experiments/smt-protocol/cases.js";
import { wasmPath } from "../../evals/experiments/smt-subset/paths.js";
import { createWasm } from "../../evals/experiments/smt-subset/wasm.js";

test("protocol cases miss and hit as named and carry N filler entries", async () => {
  const client = await createWasm(await readFile(wasmPath));
  const cases = wasmCases(client, [0, 3]);
  assert.deepEqual(
    cases.map((item) => item.name),
    ["wasm-check", "wasm-miss-0", "wasm-hit-0", "wasm-miss-3", "wasm-hit-3"],
  );
  for (const item of cases) {
    const reply = client.request(item.request);
    if (item.name === "wasm-check") {
      assert.equal(reply.kind, "solution");
      assert.equal(item.entries, 0);
      continue;
    }
    assert.equal(reply.kind, "incremental-solution", item.name);
    if (reply.kind !== "incremental-solution") continue;
    assert.equal(reply.stats.hits, item.name.includes("hit") ? 1 : 0);
    const entries = Object.keys(
      (item.request as unknown as { cache: { entries: object } }).cache.entries,
    ).length;
    assert.equal(entries, item.entries + (item.name.includes("hit") ? 1 : 0));
    assert.ok(item.cacheBytes > 0);
  }
  // Repeating a miss request never grows the snapshot it sends.
  const miss = cases.find((item) => item.name === "wasm-miss-3")!;
  const first = JSON.stringify(miss.request);
  client.request(miss.request);
  assert.equal(JSON.stringify(miss.request), first);
});
