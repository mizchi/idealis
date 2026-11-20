import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { arch, cpus, platform } from "node:os";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import type { OracleRequest } from "../../../packages/idealis/src/generated/roracle.js";
import { snapshotModule } from "../snapshot-module.js";
import { createOracle } from "../workspace-oracle/wasm.js";
import type { createNativeOracle } from "./client.js";
import { fixtures } from "./fixtures.js";
import type { SavedFixture } from "./measure.js";
import { type ObjectSample, summarizeObject } from "./object-summary.js";

interface Snapshot {
  readonly files: readonly { readonly path: string; readonly sha256: string }[];
}

const before = process.argv[2];
const output = process.argv[3];
assert.ok(before && output);
await mkdir(dirname(resolve(output)), { recursive: true });
await mkdir(output);
const save = (name: string, value: unknown) =>
  writeFile(resolve(output, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const manifest: Snapshot = JSON.parse(
  await readFile(resolve(before, "manifest.json"), "utf8"),
);
for (const file of manifest.files)
  assert.equal(
    createHash("sha256")
      .update(await readFile(resolve(before, file.path)))
      .digest("hex"),
    file.sha256,
  );
const saved = [];
for (const path of [
  "dist/evals/experiments/roracle-native/client.js",
  "dist/evals/experiments/roracle-native/canonical.js",
  "dist/evals/experiments/workspace-oracle/protocol.js",
  "dist/packages/idealis/src/trace.js",
  "evals/experiments/roracle-native/client.ts",
  "evals/experiments/roracle-native/canonical.ts",
  "tests/roracle-native/contract.ts",
  "evals/experiments/roracle-native/compare-object.ts",
  "evals/experiments/roracle-native/fixtures.ts",
  "evals/plans/native-object-profile.md",
]) {
  const destination = resolve(output, "candidate", path);
  await mkdir(dirname(destination), { recursive: true });
  await copyFile(path, destination);
  saved.push({
    path,
    sha256: createHash("sha256")
      .update(await readFile(destination))
      .digest("hex"),
  });
}
const native = resolve(output, "candidate", "roracle-native.node");
await copyFile(
  "evals/experiments/roracle-native/generated/roracle-native.node",
  native,
);
saved.push({
  path: "roracle-native.node",
  sha256: createHash("sha256")
    .update(await readFile(native))
    .digest("hex"),
});
assert.equal(
  saved.at(-1)?.sha256,
  manifest.files.find((file) => file.path === "roracle-native.node")?.sha256,
  "This experiment isolates JS changes",
);
const baseline: { readonly createNativeOracle: typeof createNativeOracle } =
  await import(snapshotModule(before, "dist", "roracle-native/client.js"));
const candidate: typeof baseline = await import(
  pathToFileURL(
    resolve(
      output,
      "candidate",
      "dist/evals/experiments/roracle-native/client.js",
    ),
  ).href
);
const isControl = process.argv.includes("--control");
const engines = [
  {
    stage: "before-object",
    client: baseline.createNativeOracle("native-object", native),
  },
  {
    stage: "after-object",
    client: (isControl ? baseline : candidate).createNativeOracle(
      "native-object",
      native,
    ),
  },
  {
    stage: "before-json",
    client: baseline.createNativeOracle("native-json", native),
  },
  {
    stage: "after-json",
    client: (isControl ? baseline : candidate).createNativeOracle(
      "native-json",
      native,
    ),
  },
];
const bytes = await readFile(
  "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
);
await writeFile(resolve(output, "roracle.wasm"), bytes, { flag: "wx" });
const wasm = createOracle(bytes);
const cases: SavedFixture[] = fixtures.flatMap((fixture) => {
  const expected = wasm.request(fixture.input);
  assert.equal(expected.status, fixture.expected);
  const copied: OracleRequest = JSON.parse(JSON.stringify(fixture.input));
  return [
    {
      name: fixture.name,
      iterations: fixture.iterations,
      input: fixture.input,
      expected,
    },
    {
      name: `${fixture.name}-copied`,
      iterations: fixture.iterations,
      input: copied,
      expected,
    },
  ];
});
await save("manifest.json", {
  host: {
    platform: platform(),
    arch: arch(),
    node: process.version,
    cpu: cpus()[0]?.model,
  },
  control: isControl,
  before: manifest,
  candidate: { files: saved },
  wasmSha256: createHash("sha256").update(bytes).digest("hex"),
  scope:
    "same native binary; JS guard before/after; codecs and full report decoder included",
});
await save("fixtures.json", cases);
const records: ObjectSample[] = [];
for (const fixture of cases) {
  for (const engine of engines)
    for (let index = 0; index < 100; index++)
      assert.deepEqual(engine.client.request(fixture.input), fixture.expected);
  for (let sample = 0; sample < 30; sample++) {
    const order = sample % 2 ? [...engines].reverse() : engines;
    for (const engine of order) {
      let totalNs = 0;
      for (let index = 0; index < fixture.iterations; index++) {
        const start = process.hrtime.bigint();
        const result = engine.client.request(fixture.input);
        totalNs += Number(process.hrtime.bigint() - start);
        assert.deepEqual(result, fixture.expected);
      }
      records.push({
        fixture: fixture.name,
        stage: engine.stage,
        sample,
        calls: fixture.iterations,
        totalNs,
      });
    }
  }
}
await save("samples.json", records);
const summary = summarizeObject(cases, records);
await save("summary.json", summary);
process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
