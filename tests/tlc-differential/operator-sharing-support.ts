import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { PortableCommand } from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import { nativePath, wasmPath } from "../ci/tlc-build.js";
import { createCiWasm, ciNativeResult } from "../ci/tlc-checkers.js";
export { nativePath, wasmPath };

export const directory = "fixtures/tlc-operator-sharing";
export const core = await createCiWasm();

export const capture = <Value>(run: () => Value) => {
  try {
    return run();
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error" as const, code: error.code, message: error.message };
  }
};

export const native = (request: PortableCommand["request"]): unknown => {
  const command = { version: 1 as const, request };
  return ciNativeResult(command, () => {
    const result = spawnSync(nativePath, [], {
      input: `${JSON.stringify(command)}\n`,
      encoding: "utf8",
      timeout: 60_000,
      maxBuffer: 16 * 1024 * 1024,
    });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
    return JSON.parse(result.stdout);
  });
};

export const normalize = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(normalize);
  if (!isRecord(value)) return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(
        ([key, entry]) =>
          !(key === "subscript" && entry === null) &&
          !(
            [
              "fairness",
              "initialProperties",
              "actionProperties",
              "properties",
            ].includes(key) &&
            Array.isArray(entry) &&
            !entry.length
          ),
      )
      .map(([key, entry]) => [key, normalize(entry)]),
  );
};

export interface Limits {
  readonly milliseconds: number;
  readonly bytes: number;
}
export interface BoundedRun {
  readonly stdout: string;
  readonly stderr: string;
  readonly exit: number | null;
  readonly signal: NodeJS.Signals | null;
  readonly elapsedMs: number;
  readonly peakRssBytes: number | null;
  readonly sampledRssBytes?: number | null;
  readonly stopped: "time" | "memory" | undefined;
}
export const mebibytes = (count: number) => count * 1024 * 1024;

export const runBounded = async (
  command: string,
  args: readonly string[],
  input: string,
  limits: Limits,
  env: NodeJS.ProcessEnv = process.env,
): Promise<BoundedRun> => {
  assert.ok(process.platform === "darwin" || process.platform === "linux");
  const directory = await mkdtemp(join(tmpdir(), "idealis-resources-"));
  const metrics = join(directory, "rusage.txt");
  try {
    const run = await new Promise<BoundedRun>((resolve, reject) => {
      const child = spawn(
        "/usr/bin/time",
        [
          ...(process.platform === "darwin" ? ["-l"] : ["-f", "%M"]),
          "-o",
          metrics,
          command,
          ...args,
        ],
        { env, detached: true, stdio: ["pipe", "pipe", "pipe"] },
      );
      const started = performance.now();
      let stdout = "";
      let stderr = "";
      let sampledRssBytes: number | null = null;
      let sampledPid: number | undefined;
      let stopped: BoundedRun["stopped"];
      child.stdout.setEncoding("utf8");
      child.stderr.setEncoding("utf8");
      child.stdout.on("data", (chunk: string) => {
        stdout += chunk;
      });
      child.stderr.on("data", (chunk: string) => {
        stderr += chunk;
      });
      child.stdin.on("error", () => undefined);
      child.stdin.end(input);
      const sample = setInterval(() => {
        if (child.pid === undefined || stopped !== undefined) return;
        if (sampledPid === undefined) {
          const descendants = spawnSync("pgrep", ["-P", String(child.pid)], {
            encoding: "utf8",
          });
          const candidate = Number.parseInt(descendants.stdout.trim(), 10);
          if (Number.isSafeInteger(candidate) && candidate > 0)
            sampledPid = candidate;
        }
        if (sampledPid !== undefined) {
          const reading = spawnSync(
            "ps",
            ["-o", "rss=", "-p", String(sampledPid)],
            { encoding: "utf8" },
          );
          const kilobytes = Number.parseInt(reading.stdout.trim(), 10);
          if (Number.isFinite(kilobytes))
            sampledRssBytes = Math.max(sampledRssBytes ?? 0, kilobytes * 1024);
        }
        if (sampledRssBytes !== null && sampledRssBytes > limits.bytes)
          stopped = "memory";
        else if (performance.now() - started > limits.milliseconds)
          stopped = "time";
        if (stopped !== undefined) {
          try {
            process.kill(-child.pid, "SIGKILL");
          } catch (error) {
            if (
              !(
                error instanceof Error &&
                "code" in error &&
                error.code === "ESRCH"
              )
            ) {
              clearInterval(sample);
              reject(error);
            }
          }
        }
      }, 10);
      child.once("error", (error) => {
        clearInterval(sample);
        reject(error);
      });
      child.once("close", (exit, signal) => {
        clearInterval(sample);
        resolve({
          stdout,
          stderr,
          exit,
          signal,
          elapsedMs: performance.now() - started,
          peakRssBytes: null,
          sampledRssBytes,
          stopped,
        });
      });
    });
    const usage = await readFile(metrics, "utf8").catch(
      (error: NodeJS.ErrnoException) => {
        if (error.code === "ENOENT") return "";
        throw error;
      },
    );
    const matched =
      process.platform === "darwin"
        ? /(?:^|\n)\s*(\d+)\s+maximum resident set size/m.exec(usage)
        : /^\s*(\d+)\s*$/m.exec(usage);
    const peak = matched?.[1];
    return {
      ...run,
      peakRssBytes:
        peak === undefined
          ? null
          : Number(peak) * (process.platform === "darwin" ? 1 : 1024),
    };
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
};

export type Outcome =
  | { readonly kind: "safety"; readonly result: Record<string, unknown> }
  | { readonly kind: "error"; readonly code: string; readonly message: string };

export interface Measured {
  readonly outcome: unknown;
  readonly elapsedMs: number;
  readonly run: BoundedRun & { readonly peakRssBytes: number };
}

export const runNative = async (
  request: PortableCommand["request"],
  limits: Limits,
): Promise<Measured> => {
  const run = await runBounded(
    nativePath,
    [],
    `${JSON.stringify({ version: 1, request })}\n`,
    limits,
  );
  assertAlive(run, limits);
  return { outcome: JSON.parse(run.stdout), elapsedMs: run.elapsedMs, run };
};

const wasmModule = new URL(
  "../../packages/idealis/src/portable-wasm.js",
  import.meta.url,
).href;
const frontendModule = new URL(
  "../../packages/idealis/src/tla-safety.js",
  import.meta.url,
).href;
const childSource = `
import { readFileSync } from "node:fs";
const { createPortableWasm } = await import(process.env.IDEALIS_WASM_MODULE);
const wasm = await createPortableWasm(readFileSync(process.env.IDEALIS_WASM_BINARY));
const job = JSON.parse(readFileSync(0, "utf8"));
const started = performance.now();
const failure = (error) => ({
  kind: "error",
  code: String(error?.code ?? ""),
  message: error?.code ? String(error.message) : String(error?.name ?? "") + ": " + String(error?.message ?? error),
});
let outcome;
try {
  let request = job.request;
  if (job.frontend === "ts") {
    const { compileTlaSafety } = await import(process.env.IDEALIS_FRONTEND_MODULE);
    const program = compileTlaSafety(job.input);
    request = { kind: "check-safety", program, bounds: job.bounds };
  }
  outcome = wasm.request({ version: 1, request });
} catch (error) {
  outcome = failure(error);
}
process.stdout.write(JSON.stringify({ elapsedMs: performance.now() - started, outcome }));
`;

const runChild = async (job: unknown, limits: Limits): Promise<Measured> => {
  const run = await runBounded(
    process.execPath,
    ["--input-type=module", "-e", childSource],
    JSON.stringify(job),
    limits,
    {
      ...process.env,
      IDEALIS_WASM_MODULE: wasmModule,
      IDEALIS_WASM_BINARY: wasmPath,
      IDEALIS_FRONTEND_MODULE: frontendModule,
    },
  );
  assertAlive(run, limits);
  const reply = JSON.parse(run.stdout);
  assert.ok(isRecord(reply));
  assert.equal(typeof reply.elapsedMs, "number");
  return { outcome: reply.outcome, elapsedMs: reply.elapsedMs as number, run };
};

export const runWasm = (
  request: PortableCommand["request"],
  limits: Limits,
): Promise<Measured> => runChild({ frontend: "wasm", request }, limits);

export const runTypeScript = (
  input: {
    readonly source: string;
    readonly config: string;
    readonly irVersion: 1 | 2;
  },
  bounds: unknown,
  limits: Limits,
): Promise<Measured> => runChild({ frontend: "ts", input, bounds }, limits);

export function assertAlive(
  run: BoundedRun,
  limits?: Limits,
): asserts run is BoundedRun & { readonly peakRssBytes: number } {
  assert.equal(
    run.stopped,
    undefined,
    `stopped on ${run.stopped}: sampled RSS ${run.sampledRssBytes ?? "unavailable"} after ${Math.round(run.elapsedMs)} ms`,
  );
  assert.equal(
    run.signal,
    null,
    `terminated by ${run.signal}: ${run.stderr.slice(0, 200)}`,
  );
  assert.equal(run.exit, 0, run.stderr.slice(0, 300));
  assert.ok(
    run.peakRssBytes !== null &&
      Number.isSafeInteger(run.peakRssBytes) &&
      run.peakRssBytes > 0,
    "OS maximum RSS measurement is missing",
  );
  if (limits)
    assert.ok(
      run.peakRssBytes <= limits.bytes,
      `OS maximum RSS ${run.peakRssBytes} exceeds ${limits.bytes} bytes`,
    );
}
