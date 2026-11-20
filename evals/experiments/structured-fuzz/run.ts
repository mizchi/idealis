import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";
import {
  archive,
  campaign,
  execute,
  type File,
  type Record,
  replayAsan,
  summarize,
  validate,
  writeCorpus,
} from "./campaign.js";
import { prepare, probes } from "./corpus.js";
import { fuzzBinary, nativePath, nightly, wasmPath } from "./paths.js";
import { digest, fingerprint } from "./provenance.js";

const phase = process.argv[2],
  directory = process.argv[3],
  parent = process.argv[4];
assert.ok(
  (phase === "development" || phase === "confirmation" || phase === "smoke") &&
    directory,
);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const stamp = await fingerprint();
const parentManifest =
  phase === "confirmation" && parent
    ? JSON.parse(await readFile(join(parent, "manifest.json"), "utf8"))
    : null;
if (phase === "confirmation") {
  assert.ok(parent);
  const previous = JSON.parse(
    await readFile(join(parent, "result.json"), "utf8"),
  );
  assert.equal(previous.status, "complete");
  assert.equal(previous.sourceVersion, stamp.sourceVersion);
  assert.equal(parentManifest?.phase, "development");
}
const seed =
  phase === "development" ? 0x39a1 : phase === "confirmation" ? 0x39c3 : 0x3901;
const runs = phase === "smoke" ? 1000 : 10_000;
const seeds = await prepare();
const initial: File[] = ["raw", "structured"].flatMap((target) =>
  seeds[target === "raw" ? "raw" : "structured"].map((bytes) => ({
    target: target === "raw" ? ("raw" as const) : ("structured" as const),
    name: digest(bytes),
    sha256: digest(bytes),
    base64: Buffer.from(bytes).toString("base64"),
  })),
);
const unique = [
  ...new Map(
    initial.map((file) => [`${file.target}/${file.name}`, file]),
  ).values(),
];
const binaries = await Promise.all(
  [nativePath, wasmPath, fuzzBinary("raw"), fuzzBinary("structured")].map(
    async (path) => ({
      path,
      sha256: digest(await readFile(path)),
      bytes: (await readFile(path)).length,
    }),
  ),
);
if (phase === "confirmation")
  assert.deepEqual(binaries, parentManifest?.binaries);
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      ...stamp,
      phase,
      seed,
      runs,
      parent,
      binaries,
      initial: unique,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      dirtyPaths: execFileSync("git", ["status", "--short"]).toString().trim(),
      environment: {
        node: process.version,
        platform: process.platform,
        arch: process.arch,
        nightly: execFileSync("rustc", [`+${nightly}`, "-Vv"])
          .toString()
          .trim(),
        cargoFuzz: execFileSync("cargo", ["fuzz", "--version"])
          .toString()
          .trim(),
      },
      interpretation:
        "Phase reach is measured on saved corpus and equal deterministic probe bytes. Corpus is coverage-selected. No full correctness, memory-model or bug absence proof",
    },
    null,
    2,
  ),
  { flag: "wx" },
);
await writeCorpus(join(directory, "corpus"), unique);
for (const target of ["raw", "structured"])
  await mkdir(join(directory, "artifacts", target), { recursive: true });
const records: Record[] = [],
  campaigns: unknown[] = [];
const start = performance.now();
try {
  const control = execute(nativePath, ["control"]);
  await writeFile(
    join(directory, "control.json"),
    JSON.stringify(control, null, 2),
    { flag: "wx" },
  );
  assert.notEqual(control.status, 0);
  assert.match(control.stderr, /protocol state differs/);
  for (const target of ["raw", "structured"] as const) {
    const result = campaign(
      target,
      join(directory, "corpus", target),
      join(directory, "artifacts", target),
      seed,
      runs,
    );
    campaigns.push({ target, ...result });
    await writeFile(
      join(directory, `${target}.json`),
      JSON.stringify(result, null, 2),
      { flag: "wx" },
    );
    assert.equal(result.status, 0, result.stderr);
    const executed = /stat::number_of_executed_units:\s*(\d+)/.exec(
      result.stderr,
    );
    assert.ok(
      executed && Number(executed[1]) >= runs,
      "Fuzz run budget did not execute",
    );
  }
  const corpus = await archive(join(directory, "corpus"));
  const artifacts = await archive(join(directory, "artifacts"));
  assert.equal(artifacts.length, 0);
  const asan = await replayAsan(join(directory, "asan-replay"), corpus, seed);
  const probeInputs = probes(
    seed,
    phase === "confirmation" ? 256 : phase === "development" ? 128 : 8,
  ).flatMap((bytes) =>
    (["raw", "structured"] as const).map((mode) => ({
      source: "probe" as const,
      input: { mode, bytes: [...bytes] },
    })),
  );
  await validate(
    [
      ...probeInputs,
      ...corpus.map((file) => ({
        source: "corpus" as const,
        input: {
          mode: file.target,
          bytes: [...Buffer.from(file.base64, "base64")],
        },
      })),
    ],
    (record) => records.push(record),
  );
  assert.equal((await fingerprint()).sourceVersion, stamp.sourceVersion);
  for (const [path, value] of [
    ["corpus.json.gz", corpus],
    ["artifacts.json.gz", artifacts],
    ["records.json.gz", records],
    ["asan-replay.json.gz", asan],
  ] as const)
    await writeFile(join(directory, path), gzipSync(JSON.stringify(value)), {
      flag: "wx",
    });
  const files = await Promise.all(
    [
      "manifest.json",
      "raw.json",
      "structured.json",
      "control.json",
      "corpus.json.gz",
      "artifacts.json.gz",
      "records.json.gz",
      "asan-replay.json.gz",
    ].map(async (path) => ({
      path,
      sha256: digest(await readFile(join(directory, path))),
    })),
  );
  const result = {
    status: "complete",
    sourceVersion: stamp.sourceVersion,
    phase,
    seed,
    runsPerTarget: runs,
    corpus: corpus.length,
    artifacts: artifacts.length,
    summary: summarize(records),
    elapsedMs: performance.now() - start,
    files,
  };
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(result, null, 2),
    { flag: "wx" },
  );
  if (phase !== "smoke")
    await appendFile(
      "evals/runs/ledger.jsonl",
      `${JSON.stringify({ runId: basename(directory), kind: "structured-fuzz", status: "complete", sourceVersion: stamp.sourceVersion })}\n`,
    );
  console.log(JSON.stringify(result));
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", message: String(error), campaigns },
      null,
      2,
    ),
    { flag: "wx" },
  );
  for (const type of ["corpus", "artifacts"])
    await writeFile(
      join(directory, `${type}-at-error.json.gz`),
      gzipSync(JSON.stringify(await archive(join(directory, type)))),
      { flag: "wx" },
    );
  if (phase !== "smoke")
    await appendFile(
      "evals/runs/ledger.jsonl",
      `${JSON.stringify({ runId: basename(directory), kind: "structured-fuzz", status: "error", sourceVersion: stamp.sourceVersion })}\n`,
    );
  throw error;
}
