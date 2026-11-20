import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { Session } from "node:inspector/promises";
import { cpus } from "node:os";
import { dirname, resolve } from "node:path";
import { summarizeCpu } from "../profile-tuning/hotspots.js";
import { decodeOracleReply } from "../workspace-oracle/protocol.js";
import { createOracle } from "../workspace-oracle/wasm.js";
import { canonicalObject } from "./canonical.js";
import { createNativeOracle, loadNativeBinding, nativePath } from "./client.js";
import { fixtures } from "./fixtures.js";

interface Phase {
  readonly name: string;
  readonly run: () => unknown;
  readonly expected: unknown;
}
interface PhaseSample {
  readonly fixture: string;
  readonly phase: string;
  readonly sample: number;
  readonly calls: number;
  readonly totalNs: number;
}
const directory = process.argv[2];
assert.ok(directory);
const mode = process.argv[3] ?? "measure";
assert.ok(["measure", "profile", "loop"].includes(mode));
await mkdir(dirname(resolve(directory)), { recursive: true });
await mkdir(directory);
const save = (name: string, value: unknown) =>
  writeFile(resolve(directory, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const binding = loadNativeBinding();
const objectClient = createNativeOracle("native-object");
const jsonClient = createNativeOracle("native-json");
const wasmBytes = await readFile(
  "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
);
const wasm = createOracle(wasmBytes);
const sourcePaths = [
  "evals/experiments/roracle-native/phases.ts",
  "evals/experiments/roracle-native/client.ts",
  "evals/experiments/roracle-native/canonical.ts",
  "evals/experiments/roracle-native/fixtures.ts",
  "evals/experiments/roracle-native/rust/src/lib.rs",
  "evals/experiments/workspace-oracle/protocol.ts",
  "evals/plans/native-object-profile.md",
];
const sources = [];
for (const path of sourcePaths) {
  const content = await readFile(path);
  sources.push({
    path,
    sha256: createHash("sha256").update(content).digest("hex"),
    content: content.toString(),
  });
}
await save("manifest.json", {
  mode,
  node: process.version,
  cpu: cpus()[0]?.model,
  sources,
  nativeSha256: createHash("sha256")
    .update(await readFile(nativePath))
    .digest("hex"),
  wasmSha256: createHash("sha256").update(wasmBytes).digest("hex"),
  scope:
    "Independent phase measurements; their medians are not an additive decomposition",
});
const records: PhaseSample[] = [];
for (const fixture of fixtures) {
  if (mode !== "measure" && fixture.name !== (process.argv[5] ?? "nested-256"))
    continue;
  const expected = wasm.request(fixture.input);
  assert.equal(expected.status, fixture.expected);
  const json = JSON.stringify(fixture.input);
  const objectReply = binding.dispatchObject(fixture.input);
  const jsonReply = binding.dispatchJson(json);
  assert.deepEqual(decodeOracleReply(objectReply), expected);
  assert.deepEqual(decodeOracleReply(jsonReply), expected);
  const phases: readonly Phase[] = [
    {
      name: "guard",
      run: () => canonicalObject(fixture.input),
      expected: undefined,
    },
    {
      name: "object-ffi",
      run: () => binding.dispatchObject(fixture.input),
      expected: objectReply,
    },
    {
      name: "json-ffi",
      run: () => binding.dispatchJson(json),
      expected: jsonReply,
    },
    {
      name: "stringify",
      run: () => JSON.stringify(fixture.input),
      expected: json,
    },
    { name: "decode", run: () => decodeOracleReply(objectReply), expected },
    {
      name: "object-request",
      run: () => objectClient.request(fixture.input),
      expected,
    },
    {
      name: "json-request",
      run: () => jsonClient.request(fixture.input),
      expected,
    },
  ];
  if (mode !== "measure") {
    const phase = phases.find(
      (phase) => phase.name === (process.argv[4] ?? "object-request"),
    );
    assert.ok(phase);
    for (let index = 0; index < 100; index++)
      assert.deepEqual(phase.run(), phase.expected);
    const session = new Session();
    if (mode === "profile") {
      session.connect();
      await session.post("Profiler.enable");
      await session.post("Profiler.setSamplingInterval", { interval: 100 });
      await session.post("Profiler.start");
    }
    for (let index = 0; index < 10000; index++)
      assert.deepEqual(phase.run(), phase.expected);
    if (mode === "profile") {
      const result = await session.post("Profiler.stop");
      session.disconnect();
      await save("profile.cpuprofile", result.profile);
      await save("summary.json", summarizeCpu(result.profile));
    }
    await save("result.json", {
      status: "matched",
      fixture: fixture.name,
      phase: phase.name,
      calls: 10000,
    });
    process.stdout.write(
      `Profiled ${fixture.name} ${phase.name}: 10000 matched calls\n`,
    );
    continue;
  }
  for (const phase of phases)
    for (let index = 0; index < 100; index++)
      assert.deepEqual(phase.run(), phase.expected);
  for (let sample = 0; sample < 30; sample++) {
    const order = [
      ...phases.slice(sample % phases.length),
      ...phases.slice(0, sample % phases.length),
    ];
    for (const phase of order) {
      let totalNs = 0;
      for (let index = 0; index < fixture.iterations; index++) {
        const start = process.hrtime.bigint();
        const result = phase.run();
        totalNs += Number(process.hrtime.bigint() - start);
        assert.deepEqual(result, phase.expected);
      }
      records.push({
        fixture: fixture.name,
        phase: phase.name,
        sample,
        calls: fixture.iterations,
        totalNs,
      });
    }
  }
}
if (mode === "measure") {
  await save("samples.json", records);
  const summary = fixtures.map((fixture) => ({
    fixture: fixture.name,
    phases: [...new Set(records.map((record) => record.phase))].map((phase) => {
      const timings = records
        .filter(
          (record) => record.fixture === fixture.name && record.phase === phase,
        )
        .map((record) => record.totalNs / record.calls)
        .sort((left, right) => left - right);
      assert.equal(timings.length, 30);
      const lower = timings[14];
      const upper = timings[15];
      assert.ok(lower && upper);
      return { phase, medianUs: (lower + upper) / 2000 };
    }),
  }));
  await save("summary.json", summary);
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
}
