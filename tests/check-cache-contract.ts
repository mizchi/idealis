import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { cases } from "../evals/experiments/rust-safety/cases.js";
import { createRtlcBackend } from "../packages/idealis/src/backend.js";
import { createHttpCheckCacheStore } from "../packages/idealis/src/check-cache-http.js";
import {
  createCheckCacheHttpHandler,
  createSqliteCheckCacheStore,
} from "../packages/idealis/src/check-cache-node.js";
import {
  CheckCacheError,
  createCheckCache,
  createMemoryCheckCacheStore,
  planCachedChecks,
  runCachedCheck,
} from "../packages/idealis/src/check-cache.js";
import type {
  CheckCacheJob,
  CheckCacheReservation,
} from "../packages/idealis/src/contracts.js";
import { createPortableWasm } from "../packages/idealis/src/portable-wasm.js";
import {
  createManifest,
  mergeReports,
} from "../packages/idealis/src/sharding.js";
import { compileTlaSafety } from "../packages/idealis/src/tla-safety.js";

const bytes = await readFile(
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
);
const core = await createPortableWasm(bytes);
const backend = createRtlcBackend(core, { version: "cache-contract-v1" });
const job: CheckCacheJob = {
  buildDigest: "a".repeat(64),
  backend: backend.descriptor,
  request: {
    format: "modelt.backend-request",
    version: 1,
    scope: {
      modelId: "counter-2",
      propertyId: "Safe",
      semantics: "tlc-1.7.4-int32",
      assumptions: [],
    },
    query: {
      kind: "explicit",
      bounds: { maxDepth: 8, maxStates: 100, maxWork: 10000 },
    },
    input: compileTlaSafety({
      source:
        "---- MODULE Cache ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=1-x\nSafe == x<2\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
    }),
  },
};
const lease = (reservation: CheckCacheReservation) => {
  assert.equal(reservation.kind, "lease");
  if (reservation.kind !== "lease") throw new Error("Expected lease");
  return reservation.lease;
};

export const registerCheckCacheContract = (
  test: (name: string, run: () => void | Promise<void>) => unknown,
): void => {
  test("signed zero is rejected before leasing but positive zero remains JSON input", async () => {
    const store = createMemoryCheckCacheStore();
    const cache = createCheckCache(store);
    for (const input of [-0, { nested: [-0] }]) {
      await assert.rejects(
        cache.acquire(
          { ...job, request: { ...job.request, input } },
          {
            owner: "a",
            runId: "zero",
            now: 0,
            leaseMs: 10,
          },
        ),
        CheckCacheError,
      );
    }
    assert.equal(
      (
        await cache.acquire(
          { ...job, request: { ...job.request, input: 0 } },
          {
            owner: "a",
            runId: "zero",
            now: 0,
            leaseMs: 10,
          },
        )
      ).kind,
      "lease",
    );
  });
  test("cleanup does not replace completed publication or the primary error", async () => {
    const cache = createCheckCache(createMemoryCheckCacheStore());
    const cleanupError = new Error("release unavailable");
    const release = async () => {
      throw cleanupError;
    };
    const options = { owner: "a", runId: "cleanup", now: () => 0, leaseMs: 10 };
    const completed = await runCachedCheck(
      { ...cache, release },
      job,
      backend,
      options,
    );
    assert.equal(completed.kind, "executed");
    const primary = new Error("backend unavailable");
    await assert.rejects(
      runCachedCheck(
        { ...createCheckCache(createMemoryCheckCacheStore()), release },
        job,
        {
          descriptor: backend.descriptor,
          check: async () => {
            throw primary;
          },
        },
        options,
      ),
      (error: unknown) =>
        error instanceof AggregateError &&
        error.cause === primary &&
        error.errors[1] === cleanupError,
    );
    const uncacheable = await runCachedCheck(
      { ...createCheckCache(createMemoryCheckCacheStore()), release },
      {
        ...job,
        request: {
          ...job.request,
          query: {
            kind: "explicit",
            bounds: { maxDepth: 0, maxStates: 100, maxWork: 10000 },
          },
        },
      },
      backend,
      {
        ...options,
        now: (() => {
          let now = -11;
          return () => (now += 11);
        })(),
      },
    );
    assert.equal(uncacheable.kind, "executed");
    assert.equal(Reflect.get(uncacheable, "cleanupError"), cleanupError);
  });
  test("a killed SQLite producer cannot publish after another process takes its lease", async () => {
    const directory = await mkdtemp(join(tmpdir(), "idealis-cache-crash-"));
    const path = join(directory, "cache.db");
    const input = join(directory, "job.json");
    await writeFile(input, JSON.stringify(job));
    const child = spawn(
      process.execPath,
      ["dist/evals/experiments/verified-cache/crash-worker.js", path, input],
      { stdio: ["pipe", "pipe", "inherit"] },
    );
    const lines = createInterface({ input: child.stdout });
    const exited = once(child, "exit");
    const store = createSqliteCheckCacheStore(path);
    try {
      const [line] = await Promise.race([
        once(lines, "line"),
        exited.then(() => {
          throw new Error("Producer exited before acquiring lease");
        }),
      ]);
      const old = JSON.parse(String(line));
      child.kill("SIGKILL");
      assert.equal((await exited)[1], "SIGKILL");
      const cache = createCheckCache(store);
      assert.equal((await cache.probe(job, 1)).kind, "busy");
      const current = lease(
        await cache.acquire(job, {
          owner: "replacement",
          runId: "replacement",
          now: 2,
          leaseMs: 10,
        }),
      );
      assert.ok(current.fence > old.fence);
      const receipt = await cache.execute(old, backend);
      assert.equal(await cache.publish(old, receipt, 3), "stale");
      await assert.rejects(cache.publish(current, receipt, 3), CheckCacheError);
      assert.equal(
        await cache.publish(current, await cache.execute(current, backend), 3),
        "published",
      );
      assert.equal((await cache.probe(job, 3)).kind, "hit");
    } finally {
      child.kill("SIGKILL");
      lines.close();
      store.close();
      await rm(directory, { recursive: true, force: true });
    }
  });

  test("empty, counterexample, checker errors and work exhaustion remain uncached", async () => {
    for (const [name, expected] of [
      ["empty", "empty"],
      ["violation", "counterexample"],
      ["mixed-equality", "error"],
    ] as const) {
      const input = cases.find((entry) => entry.name === name);
      assert.ok(input);
      const candidate: CheckCacheJob = {
        ...job,
        request: { ...job.request, input: compileTlaSafety(input) },
      };
      const cache = createCheckCache(createMemoryCheckCacheStore());
      const token = lease(
        await cache.acquire(candidate, {
          owner: "a",
          runId: name,
          now: 0,
          leaseMs: 10,
        }),
      );
      const receipt = await cache.execute(token, backend);
      assert.equal(receipt.result.outcome.kind, expected);
      assert.equal(await cache.publish(token, receipt, 1), "uncacheable");
      assert.equal((await cache.probe(candidate, 1)).kind, "miss");
    }
    const candidate: CheckCacheJob = {
      ...job,
      request: {
        ...job.request,
        query: {
          kind: "explicit",
          bounds: { maxDepth: 8, maxStates: 100, maxWork: 1 },
        },
      },
    };
    const cache = createCheckCache(createMemoryCheckCacheStore());
    const result = await runCachedCheck(cache, candidate, backend, {
      owner: "a",
      runId: "work",
      now: () => 0,
      leaseMs: 10,
    });
    assert.equal(result.kind, "executed");
    if (result.kind !== "executed") throw new Error("Expected execution");
    assert.equal(result.result.outcome.kind, "inconclusive");
    assert.equal(result.publication, "uncacheable");
  });

  test("planning budgets misses and keeps another worker's lease deferred", async () => {
    const cache = createCheckCache(createMemoryCheckCacheStore());
    await cache.acquire(job, {
      owner: "other",
      runId: "other",
      now: 0,
      leaseMs: 10,
    });
    const changed = { ...job, buildDigest: "b".repeat(64) };
    const options = {
      policy: "cost" as const,
      batchTargetMs: 4,
      maxBatchSize: 8,
    };
    const budget = { durationMs: 1000, memoryBytes: 10000, maxWorkers: 2 };
    const tasks = [
      { id: "busy", estimatedMs: 1, memoryBytes: 1, check: job },
      { id: "new", estimatedMs: 1, memoryBytes: 1, check: changed },
    ];
    const result = await planCachedChecks(cache, tasks, [], budget, options, 1);
    assert.deepEqual(
      result.deferred.map((entry) => entry.id),
      ["busy"],
    );
    assert.deepEqual(
      result.pending.map((entry) => entry.id),
      ["new"],
    );
    assert.equal(result.cached.length, 0);
    assert.notEqual(result.plan.status, "empty");
    await assert.rejects(
      planCachedChecks(
        cache,
        [
          {
            ...tasks[0],
            id: "invalid",
            check: job,
            estimatedMs: -1,
            memoryBytes: 1,
          },
        ],
        [],
        budget,
        options,
        1,
      ),
    );
  });

  test("cached planning preserves new manifest provenance and excludes hits from the budget", async () => {
    const cache = createCheckCache(createMemoryCheckCacheStore());
    const first = await createManifest(
      { workload: "cached-model", sourceVersion: "source-a" },
      [{ key: "counter", payload: job }],
    );
    const token = lease(
      await cache.acquire(job, {
        owner: "a",
        runId: first.id,
        now: 0,
        leaseMs: 10,
      }),
    );
    await cache.publish(token, await cache.execute(token, backend), 1);
    const second = await createManifest(
      { workload: "cached-model", sourceVersion: "source-b" },
      [{ key: "counter", payload: job }],
    );
    assert.notEqual(first.id, second.id);
    const task = second.tasks[0];
    assert.ok(task);
    const resolution = await planCachedChecks(
      cache,
      [
        {
          id: task.id,
          estimatedMs: 100000,
          memoryBytes: 1024 ** 3,
          check: job,
        },
      ],
      [],
      { durationMs: 0, memoryBytes: 0, maxWorkers: 1 },
      { policy: "cost", batchTargetMs: 4, maxBatchSize: 8 },
      10,
    );
    assert.equal(resolution.plan.status, "empty");
    assert.equal(resolution.plan.workers, 0);
    assert.equal(resolution.cached.length, 1);
    assert.equal(resolution.deferred.length, 0);
    assert.equal(resolution.cached[0]?.result.origin.producerRunId, first.id);
    const reports = resolution.cached.map((row) => ({
      manifestId: second.id,
      taskId: row.id,
      sourceVersion: second.sourceVersion,
      status: "ok" as const,
      result: row.result,
    }));
    assert.equal((await mergeReports(second, reports)).status, "complete");
    assert.equal(
      (
        await mergeReports(
          second,
          reports.map((row) => ({
            ...row,
            sourceVersion: first.sourceVersion,
          })),
        )
      ).status,
      "conflict",
    );
  });

  test("cache identities are immutable snapshots and corrupt persisted evidence is rejected", async () => {
    const store = createMemoryCheckCacheStore();
    const cache = createCheckCache(store);
    const mutable = JSON.parse(JSON.stringify(job));
    const options = { owner: "a", runId: "snapshot", now: 0, leaseMs: 10 };
    const acquisition = cache.acquire(mutable, options);
    options.owner = "changed";
    options.runId = "changed";
    mutable.request.input.next = [];
    const token = lease(await acquisition);
    assert.equal(token.owner, "a");
    assert.equal(token.runId, "snapshot");
    const receipt = await cache.execute(token, backend);
    assert.equal(receipt.result.outcome.kind, "explicit-complete");
    await cache.publish(token, receipt, 1);
    assert.equal((await cache.probe(mutable, 1)).kind, "miss");
    const stored = await store.read(token.key);
    assert.ok(stored?.artifact);
    Reflect.set(stored.artifact.result.outcome, "states", 99);
    const intact = await cache.probe(job, 1);
    assert.equal(intact.kind, "hit");
    if (intact.kind !== "hit") throw new Error("Expected hit");
    assert.equal(intact.artifact.result.outcome.kind, "explicit-complete");
    assert.equal(intact.artifact.result.progress?.states, 2);
    const before = await store.read(token.key);
    assert.ok(before?.artifact);
    assert.equal(
      await store.compareExchange(token.key, before.revision, {
        ...before,
        revision: before.revision + 1,
        artifact: { ...before.artifact, digest: "0".repeat(64) },
      }),
      true,
    );
    await assert.rejects(cache.probe(job, 1), CheckCacheError);
    assert.equal(await cache.evict(job), true);
    assert.equal((await cache.probe(job, 1)).kind, "miss");
    const renewed = lease(
      await cache.acquire(job, {
        owner: "a",
        runId: "after-corrupt",
        now: 1,
        leaseMs: 10,
      }),
    );
    assert.ok(renewed.fence > token.fence);
    await assert.rejects(
      cache.acquire(job, {
        owner: "a",
        runId: "overflow",
        now: Number.MAX_SAFE_INTEGER,
        leaseMs: 1,
      }),
      CheckCacheError,
    );
  });

  test("SQLite and HTTP clients share a fenced lease and persist completed results", async () => {
    const directory = await mkdtemp(join(tmpdir(), "idealis-cache-"));
    const path = join(directory, "cache.db");
    const store = createSqliteCheckCacheStore(path);
    const handler = createCheckCacheHttpHandler(store);
    const server = createServer((request, response) => {
      void handler(request, response);
    });
    server.listen(0, "127.0.0.1");
    await once(server, "listening");
    const address = server.address();
    assert.ok(address && typeof address !== "string");
    try {
      const clients = [
        createCheckCache(
          createHttpCheckCacheStore(`http://127.0.0.1:${address.port}`),
        ),
        createCheckCache(
          createHttpCheckCacheStore(`http://127.0.0.1:${address.port}`),
        ),
      ];
      const claims = await Promise.all(
        clients.map((cache, index) =>
          cache.acquire(job, {
            owner: String(index),
            runId: "http",
            now: 0,
            leaseMs: 10,
          }),
        ),
      );
      assert.deepEqual(claims.map((row) => row.kind).sort(), ["busy", "lease"]);
      const claimed = claims.find((row) => row.kind === "lease");
      assert.ok(claimed);
      const token = lease(claimed);
      const client = clients[0];
      assert.ok(client);
      assert.equal(
        await client.publish(token, await client.execute(token, backend), 1),
        "published",
      );
      const peer = createSqliteCheckCacheStore(path);
      try {
        assert.equal((await createCheckCache(peer).probe(job, 1)).kind, "hit");
      } finally {
        peer.close();
      }
    } finally {
      server.closeAllConnections();
      await new Promise<void>((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      );
      store.close();
    }
    const reopened = createSqliteCheckCacheStore(path);
    try {
      const hit = await createCheckCache(reopened).probe(job, 100);
      assert.equal(hit.kind, "hit");
      if (hit.kind !== "hit") throw new Error("Expected persisted hit");
      assert.equal(hit.artifact.runId, "http");
    } finally {
      reopened.close();
      await rm(directory, { recursive: true, force: true });
    }
  });

  test("the executor distinguishes fresh, cached and deferred model checks", async () => {
    const cache = createCheckCache(createMemoryCheckCacheStore());
    const options = {
      owner: "worker",
      runId: "executed",
      leaseMs: 10,
      now: () => 1,
    };
    const fresh = await runCachedCheck(cache, job, backend, options);
    assert.equal(fresh.kind, "executed");
    assert.equal(
      (await runCachedCheck(cache, job, backend, options)).kind,
      "cached",
    );
    await cache.evict(job);
    lease(
      await cache.acquire(job, {
        owner: "other",
        runId: "held",
        now: 1,
        leaseMs: 10,
      }),
    );
    assert.equal(
      (await runCachedCheck(cache, job, backend, options)).kind,
      "deferred",
    );
  });
  test("complete model results are reused without replaying the backend", async () => {
    const cache = createCheckCache(createMemoryCheckCacheStore());
    const token = lease(
      await cache.acquire(job, {
        owner: "a",
        runId: "run-1",
        now: 0,
        leaseMs: 10,
      }),
    );
    const receipt = await cache.execute(token, backend);
    assert.equal(receipt.result.outcome.kind, "explicit-complete");
    assert.equal(await cache.publish(token, receipt, 1), "published");
    const hit = await cache.probe(job, 100);
    assert.equal(hit.kind, "hit");
    if (hit.kind !== "hit") throw new Error("Expected hit");
    assert.deepEqual(hit.artifact.result, receipt.result);
    assert.equal(hit.artifact.runId, "run-1");
    assert.equal(
      (
        await cache.acquire(job, {
          owner: "b",
          runId: "run-2",
          now: 100,
          leaseMs: 10,
        })
      ).kind,
      "hit",
    );
  });

  test("expiry, renewal and eviction never let an earlier owner publish", async () => {
    const store = createMemoryCheckCacheStore();
    const first = createCheckCache(store);
    const second = createCheckCache(store);
    const old = lease(
      await first.acquire(job, {
        owner: "a",
        runId: "old",
        now: 0,
        leaseMs: 2,
      }),
    );
    const delayed = await first.execute(old, backend);
    assert.equal(
      (
        await second.acquire(job, {
          owner: "b",
          runId: "new",
          now: 1,
          leaseMs: 2,
        })
      ).kind,
      "busy",
    );
    const current = lease(
      await second.acquire(job, {
        owner: "b",
        runId: "new",
        now: 2,
        leaseMs: 2,
      }),
    );
    assert.ok(current.fence > old.fence);
    assert.equal(await first.publish(old, delayed, 2), "stale");
    assert.equal(await second.renew(current, 3, 10), true);
    assert.equal(
      await second.publish(current, await second.execute(current, backend), 8),
      "published",
    );
    assert.equal(await first.evict(job), true);
    const reusedOwner = lease(
      await first.acquire(job, {
        owner: "a",
        runId: "after-eviction",
        now: 9,
        leaseMs: 10,
      }),
    );
    assert.ok(reusedOwner.fence > current.fence);
    assert.equal(await first.publish(old, delayed, 9), "stale");
    assert.equal(
      await first.publish(
        reusedOwner,
        await first.execute(reusedOwner, backend),
        10,
      ),
      "published",
    );
  });

  test("concurrent clients acquire one lease and incomplete checks are never cached", async () => {
    const store = createMemoryCheckCacheStore();
    const clients = [createCheckCache(store), createCheckCache(store)];
    const results = await Promise.all(
      clients.map((cache, index) =>
        cache.acquire(job, {
          owner: String(index),
          runId: "race",
          now: 0,
          leaseMs: 10,
        }),
      ),
    );
    assert.deepEqual(results.map((result) => result.kind).sort(), [
      "busy",
      "lease",
    ]);
    const bounded: CheckCacheJob = {
      ...job,
      request: {
        ...job.request,
        query: {
          kind: "explicit",
          bounds: { maxDepth: 0, maxStates: 100, maxWork: 10000 },
        },
      },
    };
    const cache = createCheckCache(store);
    const token = lease(
      await cache.acquire(bounded, {
        owner: "a",
        runId: "bounded",
        now: 0,
        leaseMs: 10,
      }),
    );
    const receipt = await cache.execute(token, backend);
    assert.equal(receipt.result.outcome.kind, "inconclusive");
    assert.equal(await cache.publish(token, receipt, 1), "uncacheable");
    assert.equal((await cache.probe(bounded, 1)).kind, "miss");
  });

  test("dependency edits miss and a receipt from a different input is rejected", async () => {
    const cache = createCheckCache(createMemoryCheckCacheStore());
    const token = lease(
      await cache.acquire(job, {
        owner: "a",
        runId: "one",
        now: 0,
        leaseMs: 10,
      }),
    );
    const receipt = await cache.execute(token, backend);
    for (const changed of [
      { owner: "other" },
      { runId: "other" },
      { fence: token.fence + 1 },
    ])
      await assert.rejects(
        cache.publish({ ...token, ...changed }, receipt, 1),
        CheckCacheError,
      );
    await cache.publish(token, receipt, 1);
    for (const changed of [
      { ...job, buildDigest: "b".repeat(64) },
      { ...job, backend: { ...job.backend, version: "other" } },
      {
        ...job,
        request: {
          ...job.request,
          scope: { ...job.request.scope, propertyId: "Other" },
        },
      },
      { ...job, request: { ...job.request, input: { changed: true } } },
    ])
      assert.equal((await cache.probe(changed, 1)).kind, "miss");
    const changed = {
      ...job,
      request: {
        ...job.request,
        scope: { ...job.request.scope, propertyId: "Other" },
      },
    };
    const other = lease(
      await cache.acquire(changed, {
        owner: "b",
        runId: "other",
        now: 1,
        leaseMs: 10,
      }),
    );
    await assert.rejects(cache.publish(other, receipt, 2), CheckCacheError);
  });
};
