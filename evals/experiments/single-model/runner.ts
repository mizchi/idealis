import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { decodeSafetyResult } from "../../../packages/idealis/src/safety.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { normalizeTlc } from "../tlc-throughput/protocol.js";
import type { BenchmarkCase } from "./cases.js";
import { createPhaseRecorder } from "./phases.js";
import { normalizeTla } from "./tla.js";

export const routes = [
  "tlc-1",
  "tlc-2",
  "tlc-4",
  "tlc-8",
  "native",
  "wasm",
] as const;
export type Route = (typeof routes)[number];

export const prepare = async (directory: string, input: BenchmarkCase) => {
  await mkdir(directory, { recursive: false });
  await mkdir(join(directory, "tmp"));
  const moduleName = /^---- MODULE (\w+) ----/m.exec(input.source)?.[1];
  assert.ok(moduleName);
  await writeFile(join(directory, "input.json"), JSON.stringify(input), {
    flag: "wx",
  });
  await writeFile(join(directory, `${moduleName}.tla`), input.source, {
    flag: "wx",
  });
  await writeFile(join(directory, `${moduleName}.cfg`), input.config, {
    flag: "wx",
  });
};

export const runTool = async (
  route: Route | "tla-ts",
  directory: string,
  input: BenchmarkCase,
  java: string,
  jar: string,
  workers?: number,
  binary?: string,
) => {
  const moduleName = /^---- MODULE (\w+) ----/m.exec(input.source)?.[1];
  assert.ok(moduleName);
  const isTlc = route.startsWith("tlc-");
  const command = isTlc ? resolve(java) : process.execPath;
  const args = isTlc
    ? [
        "-Xmx8g",
        "-XX:+UseParallelGC",
        "-XX:ActiveProcessorCount=8",
        `-Djava.io.tmpdir=${resolve(directory, "tmp")}`,
        "-jar",
        resolve(jar),
        "-workers",
        route.slice(4),
        "-seed",
        "16",
        "-lncheck",
        "final",
        "-tool",
        "-cleanup",
        "-config",
        `${moduleName}.cfg`,
        `${moduleName}.tla`,
      ]
    : [
        "--max-old-space-size=8192",
        resolve("dist/evals/experiments/single-model/worker.js"),
        route,
        resolve(directory, "input.json"),
        ...(binary
          ? [workers === undefined ? "" : String(workers), resolve(binary)]
          : workers === undefined
            ? []
            : [String(workers)]),
      ];
  const recorder = createPhaseRecorder();
  const memory: { milliseconds: number; bytes: number }[] = [];
  let polling: Promise<void> | undefined;
  const started = performance.now();
  const child = spawn(command, args, {
    cwd: isTlc ? directory : process.cwd(),
    stdio: ["ignore", "pipe", "pipe"],
  });
  const timer = setInterval(() => {
    if (polling || !child.pid) return;
    const root = child.pid;
    polling = new Promise<void>((resolvePoll) => {
      execFile("ps", ["-axo", "pid=,ppid=,rss=,comm="], (error, stdout) => {
        if (!error) {
          const bytes = stdout.split("\n").reduce((sum, line) => {
            const fields = /^\s*(\d+)\s+(\d+)\s+(\d+)\s+(.+)$/.exec(line);
            return fields &&
              (Number(fields[1]) === root ||
                (Number(fields[2]) === root &&
                  fields[4]?.endsWith("idealis-core")))
              ? sum + Number(fields[3]) * 1024
              : sum;
          }, 0);
          if (bytes > 0)
            memory.push({ milliseconds: performance.now() - started, bytes });
        }
        resolvePoll();
      });
    }).finally(() => {
      polling = undefined;
    });
  }, 200);
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    const text = String(chunk);
    stdout += text;
    if (isTlc) recorder.add(text, performance.now() - started);
  });
  child.stderr.on("data", (chunk) => {
    stderr += String(chunk);
  });
  try {
    const exit = await new Promise<number | null>((resolveExit, reject) => {
      child.once("error", reject);
      child.once("close", resolveExit);
    });
    let output: unknown = null;
    let decodeError: string | null = null;
    try {
      if (!isTlc) output = JSON.parse(stdout);
    } catch (error) {
      decodeError = String(error);
    }
    const wallMs = performance.now() - started;
    clearInterval(timer);
    await polling;
    let phases: ReturnType<typeof recorder.finish> | null = null;
    let phaseError: string | null = null;
    try {
      if (isTlc && exit === 0 && stdout.includes("No error has been found."))
        phases = recorder.finish();
    } catch (error) {
      phaseError = String(error);
    }
    return {
      route,
      command,
      args,
      exit,
      wallMs,
      stdout,
      stderr,
      output,
      decodeError,
      memory,
      sampledMaxRssBytes:
        memory.length > 0
          ? Math.max(...memory.map((sample) => sample.bytes))
          : null,
      phases,
      phaseError,
    };
  } finally {
    clearInterval(timer);
  }
};

export const verify = (
  actual: Awaited<ReturnType<typeof runTool>>,
  input: BenchmarkCase,
  expected: "complete" | "invariant-violation",
) => {
  if (actual.route.startsWith("tlc-")) {
    if (expected === "invariant-violation") {
      assert.notEqual(actual.exit, 0);
      assert.match(actual.stdout, /Invariant Safe is violated/);
      return { status: expected };
    }
    const result = normalizeTlc(actual.stdout, actual.exit);
    assert.deepEqual(result, {
      status: expected,
      states: input.states,
      generated: input.generated,
    });
    assert.match(
      actual.stdout,
      /Finished computing initial states: 1 distinct state generated/,
    );
    assert.ok(actual.phases && actual.phases.initAndSearchMs > 0);
    return result;
  }
  assert.equal(actual.exit, 0, actual.stderr);
  assert.equal(actual.decodeError, null);
  const output = actual.output;
  assert.ok(isRecord(output));
  if (actual.route === "tla-ts") {
    assert.ok(typeof output.apiMs === "number" && output.apiMs > 0);
    return normalizeTla(output.result, input, expected);
  }
  const result = decodeSafetyResult(output.result);
  assert.equal(result.status, expected);
  if (expected === "complete") {
    assert.equal(result.states, input.states);
    assert.equal(result.generated, input.generated);
    assert.equal(result.violation, null);
  } else assert.ok(result.violation && result.violation.trace.length > 1);
  assert.ok(typeof output.apiMs === "number" && output.apiMs > 0);
  return result;
};
