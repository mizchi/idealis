import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { loadDataset } from "./dataset.js";
import { normalizeTlc, type Route, tlcSettings } from "./protocol.js";

const route = process.argv[2];
const path = process.argv[3];
const workspace = process.argv[4];
const java = process.argv[5];
const jar = process.argv[6];
assert.ok(route && path && workspace && java && jar);
const input: Awaited<ReturnType<typeof loadDataset>> = JSON.parse(
  await readFile(path, "utf8"),
);
const started = performance.now();
if (route === "tlc-1x8" || route === "tlc-8x1") {
  const settings = tlcSettings(route);
  const completed: unknown[] = [];
  let cursor = 0;
  await Promise.all(
    Array.from({ length: settings.jvmCount }, async (_, executor) => {
      while (cursor < input.jobs.length) {
        const job = input.jobs[cursor++];
        assert.ok(job);
        const task = input.parentManifest.tasks.find(
          (task) => task.id === job.id,
        );
        const source =
          task &&
          input.inputs.find((source) => source.name === task.payload.name);
        const moduleName =
          source && /^---- MODULE (\w+) ----/m.exec(source.source)?.[1];
        assert.ok(task && source && moduleName);
        const directory = resolve(workspace, job.id);
        const args = [
          `-Xmx${settings.heapGiB}g`,
          "-XX:+UseParallelGC",
          `-XX:ActiveProcessorCount=${settings.workers}`,
          `-Djava.io.tmpdir=${join(directory, "tmp")}`,
          "-jar",
          resolve(jar),
          "-workers",
          String(settings.workers),
          "-seed",
          "16",
          "-lncheck",
          "final",
          "-cleanup",
          "-config",
          `${moduleName}.cfg`,
          `${moduleName}.tla`,
        ];
        const begin = performance.now();
        const raw = await new Promise<{
          exit: number | null;
          stdout: string;
          stderr: string;
        }>((resolveResult, reject) => {
          const child = spawn(resolve(java), args, {
            cwd: directory,
            stdio: ["ignore", "pipe", "pipe"],
          });
          let stdout = "";
          let stderr = "";
          child.stdout.on("data", (data) => {
            stdout += String(data);
          });
          child.stderr.on("data", (data) => {
            stderr += String(data);
          });
          child.once("error", reject);
          child.once("close", (exit) =>
            resolveResult({ exit, stdout, stderr }),
          );
        });
        let result: unknown;
        try {
          result = normalizeTlc(raw.stdout, raw.exit);
        } catch (error) {
          completed.push({
            id: job.id,
            executor,
            args,
            ...raw,
            error: String(error),
          });
          continue;
        }
        completed.push({
          id: job.id,
          executor,
          milliseconds: performance.now() - begin,
          result,
          args,
          ...raw,
        });
        if (completed.length % 11 === 0)
          console.error(`${route}: ${completed.length}/88 tasks`);
      }
    }),
  );
  console.log(
    JSON.stringify({ route, workMs: performance.now() - started, completed }),
  );
} else {
  assert.ok(route === "native-8" || route === "wasm-8");
  const compiler = await import("../../../packages/idealis/src/tla-safety.js");
  const scheduler = await import("../../../packages/idealis/src/scheduling.js");
  const adapter = await import("../parallel-safety/node-pool.js");
  const hashing = await import("../usable/provenance.js");
  const programs = input.inputs.map((entry) => ({
    id: hashing.digest(JSON.stringify([entry.source, entry.config])),
    program: compiler.compileTlaSafety(entry),
  }));
  const pool = await adapter.createNodePool(8, {
    backend: route === "native-8" ? "native" : "wasm",
    bytes: await readFile(
      "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
    ),
    programs,
    jobs: input.jobs,
  });
  try {
    const plan = scheduler.planSchedule(
      input.jobs,
      [
        {
          id: route,
          workers: 8,
          startupMs: 0,
          costScale: 1,
          fixedMemoryBytes: 0,
        },
      ],
      {
        durationMs: Number.MAX_SAFE_INTEGER,
        memoryBytes: 8 * 1024 ** 3,
        maxWorkers: 8,
      },
      { policy: "cost", batchTargetMs: 4, maxBatchSize: 16 },
    );
    const result = await scheduler.runSchedule(plan, pool.executors);
    console.log(
      JSON.stringify({
        route: route satisfies Route,
        workMs: result.elapsedMs,
        startupMs: pool.startupMs,
        executionStatus: result.executionStatus,
        completed: result.completed.map((entry) => {
          assert.equal(entry.result.status, "ok");
          if (entry.result.status !== "ok")
            throw new Error("Idealis evaluation error");
          assert.equal(entry.result.result.violation, null);
          return {
            id: entry.id,
            result: {
              status: entry.result.result.status,
              states: entry.result.result.states,
              generated: entry.result.result.generated,
            },
          };
        }),
      }),
    );
  } finally {
    await pool.close();
  }
}
