import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cp, mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { arch, cpus, platform } from "node:os";
import { dirname, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import type { OracleRequest } from "../../../packages/idealis/src/generated/roracle.js";
import { fixtures } from "../roracle-native/fixtures.js";
import type { SavedFixture } from "../roracle-native/measure.js";
import type { OracleClient } from "../workspace-oracle/protocol.js";
import { type createEngine, type Mode, modes } from "./engines.js";

interface Engine {
  readonly mode: Mode;
  readonly client: OracleClient;
}
interface ColdSample {
  readonly mode: Mode;
  readonly sample: number;
  readonly initAndFirstUs: number;
  readonly processMs: number;
}
interface Sample {
  readonly fixture: string;
  readonly mode: string;
  readonly sample: number;
  readonly calls: number;
  readonly totalNs: number;
}
const output = process.argv[2];
assert.ok(output);
await mkdir(dirname(resolve(output)), { recursive: true });
await mkdir(output);
const save = (name: string, value: unknown) =>
  writeFile(resolve(output, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
await cp(
  "evals/experiments/roracle-wit/generated",
  resolve(output, "generated"),
  {
    recursive: true,
  },
);
await cp(
  "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
  resolve(output, "roracle.wasm"),
);
await cp(
  "evals/experiments/roracle-native/generated/roracle-native.node",
  resolve(output, "roracle-native.node"),
);
const runtime = [
  "evals/experiments/roracle-wit/engines.js",
  "evals/experiments/roracle-wit/node.js",
  "evals/experiments/roracle-wit/wit-client.js",
  "evals/experiments/roracle-wit/encode.js",
  "evals/experiments/roracle-native/client.js",
  "evals/experiments/roracle-native/canonical.js",
  "evals/experiments/workspace-oracle/protocol.js",
  "evals/experiments/workspace-oracle/wasm.js",
  "packages/idealis/src/trace.js",
];
for (const path of runtime) {
  const saved = resolve(output, "runtime", path);
  await mkdir(dirname(saved), { recursive: true });
  await cp(resolve("dist", path), saved);
}
const paths = execFileSync(
  "git",
  [
    "ls-files",
    "--cached",
    "--others",
    "--exclude-standard",
    "evals/experiments/roracle-wit",
    "tests/roracle-wit",
    "evals/experiments/roracle-native",
    "tests/roracle-native",
    "evals/experiments/workspace-oracle",
    "tests/workspace-oracle",
    "fixtures/workspace-oracle",
    "evals/plans/typed-oracle.md",
    "packages/idealis/src/generated/roracle.ts",
    "packages/idealis/src/trace.ts",
    "crates/roracle",
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
for (const path of paths) {
  const target = resolve(output, "source", path);
  await mkdir(dirname(target), { recursive: true });
  const content = await readFile(path);
  await writeFile(target, content, { flag: "wx" });
  sources.push({
    path,
    sha256: createHash("sha256").update(content).digest("hex"),
  });
}
const module: { readonly createEngine: typeof createEngine } = await import(
  pathToFileURL(
    resolve(output, "runtime/evals/experiments/roracle-wit/engines.js"),
  ).href
);
const engines: Engine[] = [];
for (const mode of modes)
  engines.push({
    mode,
    client: await module.createEngine(mode, resolve(output)),
  });
const reference = engines[0];
assert.ok(reference);
const cases: SavedFixture[] = fixtures.flatMap((fixture) => {
  const expected = reference.client.request(fixture.input);
  assert.equal(expected.status, fixture.expected);
  const copied: OracleRequest = JSON.parse(JSON.stringify(fixture.input));
  return [
    {
      name: fixture.name,
      input: fixture.input,
      iterations: fixture.iterations,
      expected,
    },
    {
      name: `${fixture.name}-copied`,
      input: copied,
      iterations: fixture.iterations,
      expected,
    },
  ];
});
await save("fixtures.json", cases);
const samples: Sample[] = [];
for (const fixture of cases) {
  for (const engine of engines)
    for (let index = 0; index < 100; index++)
      assert.deepEqual(engine.client.request(fixture.input), fixture.expected);
  for (let sample = 0; sample < 30; sample++) {
    const order = [
      ...engines.slice(sample % engines.length),
      ...engines.slice(0, sample % engines.length),
    ];
    for (const engine of order) {
      let totalNs = 0;
      for (let index = 0; index < fixture.iterations; index++) {
        const start = process.hrtime.bigint();
        const report = engine.client.request(fixture.input);
        totalNs += Number(process.hrtime.bigint() - start);
        assert.deepEqual(report, fixture.expected);
      }
      samples.push({
        fixture: fixture.name,
        mode: engine.mode,
        sample,
        calls: fixture.iterations,
        totalNs,
      });
    }
  }
}
await save("samples.json", samples);
const median = (values: readonly number[]) => {
  assert.ok(
    values.length &&
      values.every((value) => Number.isFinite(value) && value > 0),
  );
  const sorted = [...values].sort((left, right) => left - right);
  const center = sorted.length / 2;
  const lower = sorted[Math.floor(center)];
  const upper = sorted[Math.ceil(center) - 1];
  assert.ok(lower && upper);
  return (lower + upper) / 2;
};
const rows = cases.map((fixture) => ({
  fixture: fixture.name,
  modes: Object.fromEntries(
    modes.map((mode) => {
      const selected = samples.filter(
        (row) => row.fixture === fixture.name && row.mode === mode,
      );
      assert.equal(selected.length, 30);
      return [
        mode,
        median(selected.map((row) => row.totalNs / row.calls)) / 1000,
      ];
    }),
  ),
}));
const candidates = modes.slice(3).map((mode) => ({
  mode,
  versus: ["json-wasm", "native-json"].map((baseline) => {
    const ratios = rows.map((row) => {
      const before = row.modes[baseline];
      const after = row.modes[mode];
      assert.ok(before && after);
      return before / after;
    });
    const geometricMean = Math.exp(
      ratios.reduce((total, ratio) => total + Math.log(ratio), 0) /
        ratios.length,
    );
    return {
      baseline,
      geometricMean,
      worst: Math.min(...ratios),
      eligible: geometricMean >= 1.2 && Math.min(...ratios) >= 1 / 1.1,
    };
  }),
}));
const cold: ColdSample[] = [];
for (let sample = 0; sample < 10; sample++)
  for (let index = 0; index < modes.length; index++) {
    const mode = modes[(index + sample) % modes.length];
    assert.ok(mode);
    const start = process.hrtime.bigint();
    const result = JSON.parse(
      execFileSync(
        process.execPath,
        [
          "dist/evals/experiments/roracle-wit/cold.js",
          resolve(output),
          JSON.stringify(mode),
        ],
        { encoding: "utf8" },
      ),
    );
    const processMs = Number(process.hrtime.bigint() - start) / 1_000_000;
    assert.deepEqual(result.report, cases[0]?.expected);
    cold.push({
      mode,
      sample,
      initAndFirstUs: result.initAndFirstUs,
      processMs,
    });
  }
await save("cold.json", cold);
const summary = {
  rows,
  candidates,
  cold: modes.map((mode) => ({
    mode,
    initAndFirstUs: median(
      cold.filter((row) => row.mode === mode).map((row) => row.initAndFirstUs),
    ),
    processMs: median(
      cold.filter((row) => row.mode === mode).map((row) => row.processMs),
    ),
  })),
};
await save("summary.json", summary);
const files: { path: string; sha256: string }[] = [];
const collect = async (directory: string): Promise<void> => {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) await collect(path);
    else if (entry.isFile())
      files.push({
        path: relative(output, path),
        sha256: createHash("sha256")
          .update(await readFile(path))
          .digest("hex"),
      });
  }
};
await collect(resolve(output, "generated"));
await collect(resolve(output, "runtime"));
for (const path of [
  "roracle.wasm",
  "roracle-native.node",
  "fixtures.json",
  "samples.json",
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
  host: {
    platform: platform(),
    arch: arch(),
    node: process.version,
    cpu: cpus()[0]?.model,
  },
  baseRevision: execFileSync("git", ["rev-parse", "HEAD"], {
    encoding: "utf8",
  }).trim(),
  sources,
  files,
  scope:
    "canonical request packing, binding, Rust validation/check/full JSON report and common decoder; snapshot/SUT excluded",
});
process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
