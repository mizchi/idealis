import assert from "node:assert/strict";
import {
  execFileSync,
  type SpawnSyncReturns,
  spawnSync,
} from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { cpus, totalmem } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import {
  createNative,
  createNativeZ3,
  createZ3,
  maxWork,
  modelQuery,
  nativePath,
  parseZ3,
  wasmPath,
} from "./clients.js";
import { benchmarks } from "./corpus.js";
import { digest, fingerprint } from "./provenance.js";
import { createWasm } from "./wasm.js";
import { type Answer, solution } from "./wire.js";

interface Trial {
  readonly case: string;
  readonly family: string;
  readonly mode: string;
  readonly backend: string;
  readonly sample: number;
  readonly warmup: boolean;
  readonly iterations: number;
  readonly totalMs: number;
  readonly perQueryMs: number;
  readonly answer: Answer;
}
const directory = process.argv[2];
const z3Path = process.argv[3];
assert.ok(directory && z3Path);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const source = await fingerprint();
const z3WasmPath = join(
  dirname(createRequire(import.meta.url).resolve("z3-solver")),
  "z3-built.wasm",
);
const binaries = await Promise.all(
  [nativePath, wasmPath, z3Path, z3WasmPath].map(async (path) => ({
    path,
    sha256: digest(await readFile(path)),
    bytes: (await readFile(path)).length,
  })),
);
const nativeVersion = execFileSync(z3Path, ["-version"]).toString().trim();
assert.ok(nativeVersion.includes("5.4.1"));
const inputs = benchmarks();
const records: Trial[] = [];
const preflight: unknown[] = [];
const cold: unknown[] = [];
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      ...source,
      binaries,
      inputs,
      maxWork,
      nativeVersion,
      environment: {
        node: process.version,
        platform: process.platform,
        arch: process.arch,
        cpu: cpus()[0]?.model,
        logicalCpus: cpus().length,
        memoryBytes: totalmem(),
        rustc: execFileSync("rustc", ["--version"]).toString().trim(),
        hostDedicated: false,
      },
      conditions: {
        warmups: 1,
        samples: 5,
        order: "rotate backend order each sample",
        scoped:
          "Z3 declarations/logic parsed once per case; assertions parsed inside push/pop; Rust parses full source",
        plan: "0031-smt-reuse",
        primary:
          "parse+solve+complete model, including JSONL/ABI/JS API transport",
        prepared: "parse excluded, solver and model retrieval retained",
        nativeZ3SatModelCommand:
          "get-value only for independently prevalidated SAT cases; avoids an extra roundtrip",
        cold: "TTL only, 3 fresh processes each, includes Node startup for Wasm",
        validation: "outside timers",
        languageAttribution: false,
      },
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const rust = createNative();
const core = await createWasm(await readFile(wasmPath));
const z3 = await createZ3();
const nativeZ3 = createNativeZ3(z3Path);
assert.ok(z3.version.startsWith("5.4.1"));
try {
  for (const input of inputs) {
    const wasm = solution(
      core.request({ kind: "solve", source: input.source, maxWork }),
    );
    const native = solution(
      await rust.request({ kind: "solve", source: input.source, maxWork }),
    );
    const wasmZ3 = await z3.solve(input);
    const nativeReference = await nativeZ3.solve(input);
    preflight.push({
      input,
      wasm,
      native,
      z3Wasm: wasmZ3,
      z3Native: nativeReference,
    });
    assert.deepEqual(native, wasm);
    assert.equal(wasmZ3.status, input.expected);
    assert.equal(nativeReference.status, input.expected);
    assert.ok(
      wasm.status === input.expected ||
        (wasm.status === "unknown" && wasm.reason === "work"),
    );
    if (wasm.status === "sat") await z3.validate(input, wasm);
    if (nativeReference.status === "sat")
      await z3.validate(input, nativeReference);
    for (const mode of ["parse-solve", "prepared", "scoped"]) {
      const handle = core.request({ kind: "prepare", source: input.source });
      const nativeHandle = await rust.request({
        kind: "prepare",
        source: input.source,
      });
      assert.ok(handle.kind === "prepared" && nativeHandle.kind === "prepared");
      const prepared = z3.prepare(input);
      const scoped = z3.scoped(input);
      if (mode === "scoped") await nativeZ3.prepareScope(input);
      else await nativeZ3.prepare(input);
      const backends = [
        {
          name: "rust-native",
          execute: async (): Promise<Answer> =>
            solution(
              await rust.request(
                mode === "prepared"
                  ? { kind: "check", handle: nativeHandle.handle, maxWork }
                  : { kind: "solve", source: input.source, maxWork },
              ),
            ),
        },
        {
          name: "rust-wasm",
          execute: async (): Promise<Answer> =>
            solution(
              core.request(
                mode === "prepared"
                  ? { kind: "check", handle: handle.handle, maxWork }
                  : { kind: "solve", source: input.source, maxWork },
              ),
            ),
        },
        {
          name: "z3-native",
          execute: (): Promise<Answer> =>
            mode === "prepared"
              ? nativeZ3.check(input)
              : mode === "scoped"
                ? nativeZ3.scoped(input)
                : nativeZ3.solve(input),
        },
        {
          name: "z3-wasm",
          execute: (): Promise<Answer> =>
            mode === "prepared"
              ? prepared.check()
              : mode === "scoped"
                ? scoped.check(input)
                : z3.solve(input),
        },
      ];
      try {
        for (let sample = -1; sample < 5; sample++)
          for (let offset = 0; offset < backends.length; offset++) {
            const backend = backends[(offset + sample + 1) % backends.length];
            assert.ok(backend);
            const start = performance.now();
            let answer: Answer | undefined;
            for (let iteration = 0; iteration < input.iterations; iteration++)
              answer = await backend.execute();
            const totalMs = performance.now() - start;
            assert.ok(answer);
            records.push({
              case: input.name,
              family: input.family,
              mode,
              backend: backend.name,
              sample,
              warmup: sample === -1,
              iterations: input.iterations,
              totalMs,
              perQueryMs: totalMs / input.iterations,
              answer,
            });
            assert.ok(
              answer.status === input.expected ||
                (backend.name.startsWith("rust") &&
                  answer.status === "unknown"),
            );
          }
      } finally {
        prepared.close();
        scoped.close();
        core.request({ kind: "release", handle: handle.handle });
        await rust.request({ kind: "release", handle: nativeHandle.handle });
      }
    }
    console.log(`${input.name}: ${wasm.status}`);
  }
  for (const trial of records.filter(
    (trial) => !trial.warmup && trial.answer.status === "sat",
  )) {
    const input = inputs.find((input) => input.name === trial.case);
    assert.ok(input);
    await z3.validate(input, trial.answer);
  }
  const tiny = inputs[0];
  assert.ok(tiny);
  await writeFile(join(directory, "cold-input.json"), JSON.stringify(tiny), {
    flag: "wx",
  });
  for (let sample = 0; sample < 3; sample++)
    for (const backend of [
      "rust-native",
      "rust-wasm",
      "z3-native",
      "z3-wasm",
    ]) {
      const command: string =
        backend === "rust-native"
          ? nativePath
          : backend === "z3-native"
            ? z3Path
            : process.execPath;
      const args: string[] =
        backend === "z3-native"
          ? ["-in", "-smt2"]
          : backend.endsWith("wasm")
            ? [
                "dist/evals/experiments/smt-subset/cold.js",
                backend,
                resolve(directory, "cold-input.json"),
              ]
            : [];
      const input: string | undefined =
        backend === "rust-native"
          ? `${JSON.stringify({ version: 1, request: { kind: "solve", source: tiny.source, maxWork } })}\n`
          : backend === "z3-native"
            ? `${tiny.source}(check-sat)${modelQuery(tiny)}\n`
            : undefined;
      const start = performance.now();
      const result: SpawnSyncReturns<string> = spawnSync(command, args, {
        input,
        encoding: "utf8",
      });
      const totalMs = performance.now() - start;
      if (result.error) throw result.error;
      assert.equal(result.status, 0, result.stderr);
      const answer: Answer =
        backend === "z3-native"
          ? parseZ3(result.stdout, tiny)
          : backend === "rust-native"
            ? solution(JSON.parse(result.stdout))
            : JSON.parse(result.stdout);
      assert.equal(answer.status, "sat");
      await z3.validate(tiny, answer);
      cold.push({
        backend,
        sample,
        totalMs,
        answer,
        stdout: result.stdout,
        stderr: result.stderr,
      });
    }
  assert.equal((await fingerprint()).sourceVersion, source.sourceVersion);
  await writeFile(
    join(directory, "preflight.json"),
    JSON.stringify(preflight, null, 2),
    { flag: "wx" },
  );
  await writeFile(
    join(directory, "trials.json"),
    JSON.stringify(records, null, 2),
    { flag: "wx" },
  );
  await writeFile(join(directory, "cold.json"), JSON.stringify(cold, null, 2), {
    flag: "wx",
  });
  const medians = inputs.flatMap((input) =>
    ["parse-solve", "prepared", "scoped"].map((mode) => ({
      case: input.name,
      family: input.family,
      mode,
      backends: Object.fromEntries(
        ["rust-native", "rust-wasm", "z3-native", "z3-wasm"].map((backend) => {
          const samples = records.filter(
            (trial) =>
              trial.case === input.name &&
              trial.mode === mode &&
              trial.backend === backend &&
              !trial.warmup,
          );
          const times = samples
            .map((trial) => trial.perQueryMs)
            .sort((left, right) => left - right);
          return [
            backend,
            {
              medianMs: times[2],
              minMs: times[0],
              maxMs: times[4],
              statuses: [
                ...new Set(samples.map((trial) => trial.answer.status)),
              ],
            },
          ];
        }),
      ),
    })),
  );
  const result = {
    status: "complete",
    sourceVersion: source.sourceVersion,
    z3Version: z3.version,
    measuredSamples: records.filter((trial) => !trial.warmup).length,
    warmups: records.filter((trial) => trial.warmup).length,
    coldSamples: cold.length,
    medians,
    files: await Promise.all(
      [
        "manifest.json",
        "preflight.json",
        "trials.json",
        "cold.json",
        "cold-input.json",
      ].map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  };
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(result, null, 2),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "smt-subset-benchmark", status: "complete", sourceVersion: source.sourceVersion })}\n`,
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", message: String(error), preflight, records, cold },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "smt-subset-benchmark", status: "error", sourceVersion: source.sourceVersion })}\n`,
  );
  throw error;
} finally {
  await Promise.all([rust.close(), nativeZ3.close(), z3.close()]);
}
