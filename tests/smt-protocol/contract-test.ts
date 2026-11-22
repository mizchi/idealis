import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  buildDigest,
  chain,
  closeRequest,
  freshRequest,
  wasmCases,
} from "../../evals/experiments/smt-protocol/cases.js";
import { createNative } from "../../evals/experiments/smt-subset/clients.js";
import { wasmPath } from "../../evals/experiments/smt-subset/paths.js";
import { createWasm } from "../../evals/experiments/smt-subset/wasm.js";
import type {
  Reply,
  Request,
} from "../../evals/experiments/smt-subset/wire.js";

test("protocol cases miss and hit as named and carry N filler entries", async () => {
  const client = await createWasm(await readFile(wasmPath));
  const cases = wasmCases(client, [0, 3]);
  assert.deepEqual(
    cases.map((item) => item.name),
    [
      "wasm-check",
      ...[0, 3].flatMap((count) =>
        ["miss", "hit", "handle-hit", "handle-miss"].map(
          (kind) => `wasm-${kind}-${count}`,
        ),
      ),
    ],
  );
  for (const item of cases) {
    if (item.name.includes("handle")) {
      const request = item.fresh ? freshRequest(client, item) : item.request;
      const reply = client.request(request);
      assert.ok(reply.kind === "incremental-solution", item.name);
      assert.deepEqual(
        [reply.stats.hits, reply.stats.writes],
        item.fresh ? [0, 1] : [1, 0],
        item.name,
      );
      if (item.fresh) {
        // The fresh store holds the N fillers plus the miss just written.
        const exported = client.request({
          kind: "cache-export",
          cacheHandle: reply.cacheHandle!,
        });
        assert.ok(exported.kind === "cache-snapshot");
        assert.equal(
          Object.keys(exported.cache.entries as object).length,
          item.entries + 1,
        );
        closeRequest(client, request);
      }
      continue;
    }
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

test("native and Wasm keep a cache store in place and agree on every reply", async () => {
  const wasm = await createWasm(await readFile(wasmPath));
  const native = createNative();
  const transcript = async (
    send: (request: Request) => Promise<Reply> | Reply,
  ) => {
    const replies: Reply[] = [];
    const call = async (request: Request) => {
      const reply = await send(request);
      replies.push(reply);
      return reply;
    };
    const opened = await call({
      kind: "cache-open",
      buildDigest,
      maxEntries: 8,
    });
    assert.equal(opened.kind, "cache-opened");
    if (opened.kind !== "cache-opened") return replies;
    const { cacheHandle } = opened;
    const prepared = await call({ kind: "prepare", source: chain(4) });
    assert.equal(prepared.kind, "prepared");
    if (prepared.kind !== "prepared") return replies;
    const check: Request = {
      kind: "check-incremental",
      handle: prepared.handle,
      maxWork: 100_000,
      cacheHandle,
    };
    const miss = await call(check);
    const hit = await call(check);
    assert.ok(
      miss.kind === "incremental-solution" &&
        hit.kind === "incremental-solution",
    );
    assert.deepEqual([miss.stats.writes, hit.stats.hits], [1, 1]);
    assert.equal(hit.cacheHandle, cacheHandle);
    assert.equal(hit.cache, undefined);
    const script = await call({
      kind: "run-script-incremental",
      source: "(declare-const a Int)(assert (<= a 3))(check-sat)(check-sat)",
      maxWork: 100_000,
      cacheHandle,
    });
    assert.ok(script.kind === "incremental-script-results");
    assert.deepEqual(script.stats, {
      hits: 1,
      misses: 1,
      writes: 1,
      invalidated: false,
    });
    const exported = await call({ kind: "cache-export", cacheHandle });
    assert.ok(exported.kind === "cache-snapshot");
    const reopened = await call({
      kind: "cache-open",
      buildDigest,
      maxEntries: 8,
      cache: exported.cache,
    });
    assert.ok(reopened.kind === "cache-opened");
    assert.equal(reopened.entries, 2);
    assert.equal(
      (await call({ kind: "cache-close", cacheHandle })).kind,
      "released",
    );
    const closed = await call(check);
    assert.ok(closed.kind === "error");
    assert.equal(closed.message, "unknown cache handle");
    const mixed = await call({
      ...check,
      cacheHandle: reopened.cacheHandle,
      enabled: true,
    });
    assert.ok(mixed.kind === "error");
    assert.equal(mixed.message, "cacheHandle excludes snapshot fields");
    return replies;
  };
  try {
    assert.deepEqual(
      await transcript(native.request),
      await transcript(wasm.request),
    );
  } finally {
    await native.close();
  }
});
