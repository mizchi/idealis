import assert from "node:assert/strict";
import { type SpawnSyncReturns, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { SafetyOptions, SafetyProgram } from "../../src/contracts.js";

interface Fixture {
  readonly name: string;
  readonly states: number;
  readonly generated: number;
  readonly program: SafetyProgram;
  readonly options?: SafetyOptions;
}
const [binary, directory] = process.argv.slice(2);
assert.ok(binary && directory);
assert.equal(
  process.platform,
  "darwin",
  "RSS parser uses macOS time -l byte units",
);
await mkdir(directory);
const digest = (bytes: Buffer) =>
  createHash("sha256").update(bytes).digest("hex");
const paths = [
  "crates/rtlc/src/symmetry-fixtures.json",
  "crates/rtlc/src/reduction-fixtures.json",
];
const fixtures: Fixture[] = (
  await Promise.all(
    paths.map(
      async (path): Promise<Fixture[]> =>
        JSON.parse(await readFile(path, "utf8")),
    ),
  )
).flat();
const inputs = fixtures.filter((input) =>
  ["counter-7-5-", "record-5-", "record-12-", "monotone-10/"].some((prefix) =>
    input.name.startsWith(prefix),
  ),
);
assert.equal(inputs.length, 12);
const fingerprints = await Promise.all(
  [
    binary,
    ...paths,
    "evals/native-resources/run.ts",
    "evals/plans/0061-native-resources.md",
  ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const save = async (path: string, value: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  plan: "0061-native-resources",
  fingerprints,
  inputs,
  warmup: 1,
  measured: 3,
  rssUnit: "bytes",
  scope:
    "Rust startup, compiled IR JSON decode, check and serialization; no Node host or TLA frontend",
});
const samples = [];
for (const iteration of [0, 1, 2, 3]) {
  const order = inputs.map(
    (_, index) => inputs[(index + iteration) % inputs.length],
  );
  for (const input of iteration % 2 ? order.reverse() : order) {
    assert.ok(input);
    const request = {
      version: 1,
      request: {
        kind: "check-safety",
        program: input.program,
        bounds: { maxDepth: 100000, maxStates: 1000000, maxWork: 1000000000 },
        ...(input.options ? { options: input.options } : {}),
      },
    };
    const started = performance.now();
    const result: SpawnSyncReturns<string> = spawnSync(
      "/usr/bin/time",
      ["-l", resolve(binary)],
      {
        input: `${JSON.stringify(request)}\n`,
        encoding: "utf8",
        maxBuffer: 8 * 1024 * 1024,
      },
    );
    const wallMs = performance.now() - started;
    const response: {
      kind: string;
      result?: { status: string; states: number; generated: number };
    } = JSON.parse(result.stdout);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(response.kind, "safety");
    assert.equal(response.result?.status, "complete");
    assert.equal(response.result?.states, input.states);
    assert.equal(response.result?.generated, input.generated);
    const rss = /^\s*(\d+)\s+maximum resident set size\s*$/m.exec(
      result.stderr,
    );
    assert.ok(rss, result.stderr);
    const sample = {
      iteration,
      name: input.name,
      warmup: iteration === 0,
      wallMs,
      maximumRssBytes: Number(rss[1]),
      stdout: result.stdout,
      stderr: result.stderr,
    };
    samples.push(sample);
    await save(`${iteration}-${input.name.replaceAll("/", "-")}.json`, sample);
  }
}
for (const file of fingerprints)
  assert.equal(digest(await readFile(file.path)), file.sha256, file.path);
await save("result.json", { status: "complete", samples });
console.log(`Saved ${samples.length} fresh native process measurements`);
