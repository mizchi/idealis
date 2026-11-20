import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { Session } from "node:inspector/promises";
import { cpus } from "node:os";
import { join, resolve } from "node:path";
import { summarizeCpu } from "../profile-tuning/hotspots.js";
import { snapshotModule } from "../snapshot-module.js";
import type { compareOracle } from "../workspace-oracle/node.js";
import type { snapshot } from "../workspace-oracle/snapshot.js";
import type { createOracle } from "../workspace-oracle/wasm.js";
import { readOracleWasm } from "./wasm.js";

interface Phase {
  readonly name: string;
  readonly run: () => unknown;
  readonly expected: unknown;
}

const root = process.argv[2];
const output = process.argv[3];
const mode = process.argv[4] ?? "measure";
assert.ok(root && output);
assert.ok(mode === "profile" || mode === "measure");
const directory = resolve(output);
const frozen = resolve(root);
process.env.IDEALIS_ORACLE_BACKEND = "wasm";
await mkdir(directory);
const save = async (name: string, value: unknown) =>
  writeFile(join(directory, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const manifest = JSON.parse(
  await readFile(join(frozen, "manifest.json"), "utf8"),
);
for (const file of manifest.files)
  assert.equal(
    createHash("sha256")
      .update(await readFile(join(frozen, file.path)))
      .digest("hex"),
    file.sha256,
  );
const snapshots: { snapshot: typeof snapshot } = await import(
  snapshotModule(frozen, "dist", "workspace-oracle/snapshot.js")
);
const wasm: { createOracle: typeof createOracle } = await import(
  snapshotModule(frozen, "dist", "workspace-oracle/wasm.js")
);
const node: { compareOracle: typeof compareOracle } = await import(
  snapshotModule(frozen, "dist", "workspace-oracle/node.js")
);
const binary = await readOracleWasm(frozen);
const core = wasm.createOracle(binary);
process.chdir(frozen);
const cases: readonly {
  id: string;
  request: { actual: unknown; expected: unknown };
}[] = JSON.parse(await readFile(join(frozen, "cases.json"), "utf8"));
await save("manifest.json", {
  plan: "0055",
  frozen,
  artifacts: manifest,
  node: process.version,
  cpu: cpus()[0]?.model,
  mode,
  samples: 20,
  warmup: 75,
  iterations: 25,
});
if (mode === "profile") {
  const input = cases[0];
  assert.ok(input);
  for (let index = 0; index < 75; index++)
    node.compareOracle(input.request.actual, input.request.expected);
  const session = new Session();
  session.connect();
  await session.post("Profiler.enable");
  await session.post("Profiler.setSamplingInterval", { interval: 100 });
  await session.post("Profiler.start");
  for (let index = 0; index < 5000; index++)
    assert.equal(
      node.compareOracle(input.request.actual, input.request.expected).status,
      "matched",
    );
  const result = await session.post("Profiler.stop");
  session.disconnect();
  await save("profile.cpuprofile", result.profile);
  await save("summary.json", summarizeCpu(result.profile));
  await save("result.json", {
    status: "complete",
    measurement: false,
    calls: 5000,
    case: input.id,
  });
} else {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const instance = new WebAssembly.Instance(new WebAssembly.Module(binary), {});
  const memory = instance.exports.memory;
  const resize = instance.exports.oracle_input_resize;
  const dispatch = instance.exports.oracle_dispatch;
  const pointer = instance.exports.oracle_output_ptr;
  const length = instance.exports.oracle_output_len;
  assert.ok(
    memory instanceof WebAssembly.Memory &&
      typeof resize === "function" &&
      typeof dispatch === "function" &&
      typeof pointer === "function" &&
      typeof length === "function",
  );
  const records = [];
  for (const input of [
    ...cases,
    {
      id: "mismatch",
      request: { actual: { value: 8 }, expected: { value: 7 } },
    },
    {
      id: "unicode",
      request: {
        actual: { text: "雪😀" },
        expected: { text: "雪😀" },
      },
    },
  ]) {
    const pair = () => [
      snapshots.snapshot(input.request.expected),
      snapshots.snapshot(input.request.actual),
    ];
    const expectedPair = pair();
    const batch = {
      version: 1,
      group: "workspace-equality",
      reference: "expected",
      implementations: ["actual"],
      cases: [
        {
          id: "case",
          input: { label: null },
          reference: { kind: "value", value: expectedPair[0] },
          actual: [{ kind: "value", value: expectedPair[1] }],
        },
      ],
    };
    const serialized = JSON.stringify(batch);
    const data = encoder.encode(serialized);
    const start = resize(data.length);
    new Uint8Array(memory.buffer, start, data.length).set(data);
    dispatch();
    const reply: Uint8Array = new Uint8Array(
      memory.buffer,
      pointer(),
      length(),
    ).slice();
    const decoded: unknown = JSON.parse(decoder.decode(reply));
    assert.deepEqual(
      core.request(batch),
      node.compareOracle(input.request.actual, input.request.expected),
    );
    const phases: readonly Phase[] = [
      { name: "snapshot-pair", run: pair, expected: expectedPair },
      {
        name: "stringify",
        run: () => JSON.stringify(batch),
        expected: serialized,
      },
      { name: "encode", run: () => encoder.encode(serialized), expected: data },
      {
        name: "wasm-dispatch",
        run: () => {
          dispatch();
          return length();
        },
        expected: reply.length,
      },
      {
        name: "decode-parse",
        run: () => JSON.parse(decoder.decode(reply)),
        expected: decoded,
      },
      {
        name: "whole",
        run: () =>
          node.compareOracle(input.request.actual, input.request.expected),
        expected: core.request(batch),
      },
    ];
    for (const reverse of [false, true])
      for (const phase of reverse ? [...phases].reverse() : phases) {
        for (let index = 0; index < 75; index++)
          assert.deepEqual(phase.run(), phase.expected);
        const samples = [];
        for (let sample = 0; sample < 20; sample++) {
          const observations = [];
          const clock = process.hrtime.bigint();
          for (let iteration = 0; iteration < 25; iteration++)
            observations.push(phase.run());
          const elapsed = Number(process.hrtime.bigint() - clock);
          for (const observation of observations)
            assert.deepEqual(observation, phase.expected);
          samples.push(elapsed);
        }
        records.push({
          id: input.id,
          phase: phase.name,
          reverse,
          samplesNs: samples,
        });
      }
  }
  await save("result.json", { status: "complete", measurement: true, records });
}
