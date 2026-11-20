import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { Session } from "node:inspector/promises";
import { cpus, loadavg } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { gzipSync } from "node:zlib";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import {
  checkSafety,
  decodeSafetyResult,
} from "../../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { bounds } from "../single-model/cases.js";
import { digest } from "../usable/provenance.js";
import { inputs } from "./inputs.js";
import { fingerprint } from "./provenance.js";
import { executableDigest } from "./wasm-names.js";

const backend = process.argv[2];
const binary = process.argv[3];
const directory = process.argv[4];
assert.ok((backend === "native" || backend === "wasm") && binary && directory);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const source = await fingerprint();
const sha256 = digest(await readFile(binary));
const files: string[] = [];
const records: unknown[] = [];
const write = async (path: string, value: unknown) => {
  await writeFile(
    join(directory, path),
    `${JSON.stringify(value, null, 2)}\n`,
    { flag: "wx" },
  );
  files.push(path);
};
try {
  let sameWasmCode: boolean | null = null;
  if (backend === "wasm") {
    sameWasmCode =
      executableDigest(new Uint8Array(await readFile(binary))) ===
      executableDigest(
        new Uint8Array(
          await readFile(
            "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
          ),
        ),
      );
    assert.equal(
      sameWasmCode,
      true,
      "Named Wasm must have exactly the same noncustom sections",
    );
  }
  await write("manifest.json", {
    plan: "0024-profile-tuning",
    ...source,
    backend,
    binary,
    sha256,
    sameWasmCode,
    commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    startedAt: new Date().toISOString(),
    node: process.version,
    cpu: cpus()[0]?.model,
    loadAverage: loadavg(),
    samplingIntervalUs: 1000,
    repetitions: 16,
    profileBuild:
      backend === "native"
        ? "release opt-level=3/lto=true/codegen-units=1, CARGO_PROFILE_RELEASE_STRIP=none"
        : "production bytes + validated name section from cargo rustc --release -- -C strip=none",
    timing: "Profile observations only, not performance measurements",
  });
  for (const input of inputs()) {
    const program = compileTlaSafety(input);
    await write(`${input.name}-input.json`, { ...input, program, bounds });
    if (backend === "wasm") {
      const core = await createPortableWasm(await readFile(binary));
      const session = new Session();
      session.connect();
      await session.post("Profiler.enable");
      await session.post("Profiler.setSamplingInterval", { interval: 1000 });
      await session.post("Profiler.start");
      const results = [];
      for (let iteration = 0; iteration < 16; iteration++) {
        const result = checkSafety(core, program, bounds);
        assert.equal(result.status, "complete");
        assert.equal(result.states, input.states);
        assert.equal(result.generated, input.generated);
        results.push({
          states: result.states,
          generated: result.generated,
          work: result.work,
        });
      }
      const output = await session.post("Profiler.stop");
      assert.ok(
        output.profile.samples?.length,
        "CPU profile must contain samples",
      );
      session.disconnect();
      const path = `${input.name}.cpuprofile.gz`;
      await writeFile(
        join(directory, path),
        gzipSync(JSON.stringify(output.profile)),
        { flag: "wx" },
      );
      files.push(path);
      records.push({ input: input.name, path, results });
    } else {
      for (const workers of [1, 8]) {
        const name = `${input.name}-${workers}`;
        const path = `${name}.json.gz`;
        const args = [
          "record",
          "--save-only",
          "--unstable-presymbolicate",
          "--rate",
          "1000",
          "--output",
          resolve(directory, path),
          resolve(binary),
        ];
        const command = {
          version: 1,
          request: { kind: "check-safety-parallel", program, bounds, workers },
        };
        const requests = `${JSON.stringify(command)}\n`.repeat(16);
        await writeFile(join(directory, `${name}-requests.jsonl`), requests, {
          flag: "wx",
        });
        files.push(`${name}-requests.jsonl`);
        const child = spawn("samply", args, {
          env: { ...process.env, DYLD_LIBRARY_PATH: "/usr/lib" },
          stdio: ["pipe", "pipe", "pipe"],
        });
        let stdout = "";
        let stderr = "";
        child.stdout.on("data", (chunk) => {
          stdout += String(chunk);
        });
        child.stderr.on("data", (chunk) => {
          stderr += String(chunk);
        });
        child.stdin.on("error", () => {});
        child.stdin.end(requests);
        const exit = await new Promise<number | null>((resolveExit, reject) => {
          child.once("error", reject);
          child.once("close", resolveExit);
        });
        await write(`${name}-process.json`, {
          command: "samply",
          args,
          environment: { DYLD_LIBRARY_PATH: "/usr/lib" },
          exit,
          stdout,
          stderr,
        });
        assert.equal(exit, 0, stderr);
        const results = stdout
          .trim()
          .split("\n")
          .map((line) => {
            const response: unknown = JSON.parse(line);
            assert.ok(isRecord(response) && response.kind === "safety");
            const result = decodeSafetyResult(response.result);
            assert.equal(result.status, "complete");
            assert.equal(result.states, input.states);
            assert.equal(result.generated, input.generated);
            return {
              states: result.states,
              generated: result.generated,
              work: result.work,
            };
          });
        assert.equal(results.length, 16);
        files.push(path, `${name}.json.syms.json`);
        records.push({ input: input.name, workers, path, results });
      }
    }
    console.log(`Profiled ${backend}/${input.name}`);
  }
  assert.equal(
    (await fingerprint()).sourceVersion,
    source.sourceVersion,
    "Sources changed while profiling",
  );
  assert.equal(digest(await readFile(binary)), sha256);
  await write("result.json", {
    status: "complete",
    sourceVersion: source.sourceVersion,
    records,
    files: await Promise.all(
      files.map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "cpu-profile", backend, status: "complete", sourceVersion: source.sourceVersion })}\n`,
  );
} catch (error) {
  await write("error.json", {
    status: "error",
    message: String(error),
    records,
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "cpu-profile", backend, status: "error", sourceVersion: source.sourceVersion })}\n`,
  );
  throw error;
}
