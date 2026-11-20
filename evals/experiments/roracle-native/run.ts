import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { arch, cpus, platform, release } from "node:os";
import { dirname, resolve } from "node:path";
import { nativePath } from "./client.js";
import { fixtures } from "./fixtures.js";
import {
  type ColdSample,
  createClient,
  modes,
  type Sample,
  type SavedFixture,
  summarize,
} from "./measure.js";

const output = process.argv[2];
assert.ok(output, "Provide a new immutable output directory");
await mkdir(dirname(resolve(output)), { recursive: true });
await mkdir(output);
const save = async (name: string, value: unknown) => {
  await writeFile(
    resolve(output, name),
    `${JSON.stringify(value, null, 2)}\n`,
    { flag: "wx" },
  );
};
const wasmPath = resolve(output, "roracle.wasm");
const addonPath = resolve(output, "roracle-native.node");
await copyFile(
  "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
  wasmPath,
);
await copyFile(nativePath, addonPath);
const sourcePaths = execFileSync(
  "git",
  [
    "ls-files",
    "--cached",
    "--others",
    "--exclude-standard",
    "crates/roracle",
    "packages/idealis/src/generated/roracle.ts",
    "packages/idealis/src/trace.ts",
    "evals/experiments/workspace-oracle",
    "tests/workspace-oracle",
    "fixtures/workspace-oracle",
    "evals/experiments/roracle-native",
    "tests/roracle-native",
    "scripts/oracle-types.ts",
    "scripts/build-native-oracle.ts",
    "evals/plans/native-oracle.md",
    "justfile",
    "package.json",
    "packages/idealis/package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
  ],
  { encoding: "utf8" },
)
  .trim()
  .split("\n");
const sources = [];
for (const path of sourcePaths) {
  const content = await readFile(path);
  const saved = resolve(output, "source", path);
  await mkdir(dirname(saved), { recursive: true });
  await writeFile(saved, content, { flag: "wx" });
  sources.push({
    path,
    sha256: createHash("sha256").update(content).digest("hex"),
  });
}
const wasm = await readFile(wasmPath);
const clients = modes.map((mode) => ({
  mode,
  client: createClient(mode, wasm, addonPath),
}));
const reference = clients[0];
assert.ok(reference);
const saved: SavedFixture[] = fixtures.map((fixture) => {
  const expected = reference.client.request(fixture.input);
  assert.equal(expected.status, fixture.expected);
  return {
    name: fixture.name,
    iterations: fixture.iterations,
    input: fixture.input,
    expected,
  };
});
await save("fixtures.json", saved);
const warm: Sample[] = [];
for (const fixture of saved) {
  for (const engine of clients)
    for (let index = 0; index < 100; index++)
      assert.deepEqual(engine.client.request(fixture.input), fixture.expected);
  for (let sample = 0; sample < 30; sample++) {
    const order = [
      ...clients.slice(sample % clients.length),
      ...clients.slice(0, sample % clients.length),
    ];
    for (const engine of order) {
      let totalNs = 0;
      for (let iteration = 0; iteration < fixture.iterations; iteration++) {
        const start = process.hrtime.bigint();
        const report = engine.client.request(fixture.input);
        totalNs += Number(process.hrtime.bigint() - start);
        assert.deepEqual(report, fixture.expected);
      }
      warm.push({
        fixture: fixture.name,
        mode: engine.mode,
        sample,
        calls: fixture.iterations,
        totalNs,
      });
    }
  }
}
await save("warm.json", warm);
const cold: ColdSample[] = [];
for (let sample = 0; sample < 10; sample++) {
  for (let index = 0; index < modes.length; index++) {
    const mode = modes[(index + sample) % modes.length];
    assert.ok(mode);
    const start = process.hrtime.bigint();
    const reply = execFileSync(
      process.execPath,
      [
        "dist/evals/experiments/roracle-native/cold.js",
        mode,
        wasmPath,
        addonPath,
        resolve(output, "fixtures.json"),
      ],
      { encoding: "utf8" },
    );
    const processMs = Number(process.hrtime.bigint() - start) / 1_000_000;
    const result = JSON.parse(reply);
    assert.deepEqual(result.report, saved[0]?.expected);
    cold.push({
      mode,
      sample,
      initAndFirstUs: result.initAndFirstUs,
      processMs,
    });
  }
}
await save("cold.json", cold);
const summary = summarize(saved, warm, cold);
await save("summary.json", summary);
const files = [];
for (const path of [
  "roracle.wasm",
  "roracle-native.node",
  "fixtures.json",
  "warm.json",
  "cold.json",
  "summary.json",
])
  files.push({
    path,
    sha256: createHash("sha256")
      .update(await readFile(resolve(output, path)))
      .digest("hex"),
  });
await save("manifest.json", {
  version: 1,
  host: {
    platform: platform(),
    arch: arch(),
    release: release(),
    cpu: cpus()[0]?.model,
    node: process.version,
  },
  rustc: execFileSync("rustc", ["--version"], { encoding: "utf8" }).trim(),
  revision: execFileSync("git", ["rev-parse", "HEAD"], {
    encoding: "utf8",
  }).trim(),
  scope:
    "request incl codecs and decoder; snapshot/SUT excluded; object input + JSON output",
  files,
  sources,
});
process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
