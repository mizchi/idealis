import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  appendFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { fromPortableValue } from "../../../packages/idealis/src/portable-value.js";
import { decodeSafetyResult } from "../../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { digest } from "../usable/provenance.js";
import { type Case, cases } from "./cases.js";
import { classify, itfStates, validateWitness } from "./results.js";

const directory = process.argv[2];
const java = process.argv[3];
const jar = process.argv[4];
assert.ok(directory && java && jar);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const configFile = resolve(join(directory, "apalache.json"));
await writeFile(configFile, "{}\n", { flag: "wx" });
const base = await fingerprint();
const extra = execFileSync("git", [
  "ls-files",
  "-z",
  "--cached",
  "--others",
  "--exclude-standard",
  "--",
  "evals/experiments/apalache-comparison",
  "tests/apalache-comparison",
  "evals/plans/0027-apalache-comparison.md",
])
  .toString()
  .split("\0")
  .filter(Boolean);
const sources = [
  ...base.sources,
  ...(await Promise.all(
    extra.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  )),
].sort((left, right) => left.path.localeCompare(right.path));
const sourceVersion = digest(JSON.stringify(sources));
const binaryPaths = [
  java,
  jar,
  "crates/idealis-core/target/release/idealis-core",
];
const binaries = await Promise.all(
  binaryPaths.map(async (path) => ({
    path,
    sha256: digest(await readFile(path)),
  })),
);
const version = execFileSync(java, ["-jar", resolve(jar), "version"])
  .toString()
  .trim();
assert.equal(version, "0.63.0");
const javaVersion = spawnSync(java, ["-version"], { encoding: "utf8" });
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "0027-apalache-comparison",
      sourceVersion,
      sources,
      binaries,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      environment: {
        node: process.version,
        platform: `${process.platform}/${process.arch}`,
        java: javaVersion.stderr,
        apalache: version,
      },
      conditions: {
        warmups: 1,
        repetitions: 3,
        apalacheHeap: "4096m",
        algo: "incremental",
        solver: "z3",
        encoding: "oopsla19",
        rtlcWorkers: 1,
        timeout: null,
        wallIncludes:
          "fresh process, input decode and result classification; rtlc-source also starts Node and compiles TLA; witness validation and archive I/O excluded",
      },
      startedAt: new Date().toISOString(),
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const inputs = cases.map((input) => ({
  ...input,
  bounds: { maxDepth: input.depth, maxStates: 65_536, maxWork: 2_000_000 },
  program: compileTlaSafety(input),
}));
await writeFile(
  join(directory, "inputs.json"),
  JSON.stringify(inputs, null, 2),
  { flag: "wx" },
);
const files = async (path: string): Promise<string[]> => {
  const entries = await readdir(path, { withFileTypes: true });
  return (
    await Promise.all(
      entries.map((entry) =>
        entry.isDirectory()
          ? files(join(path, entry.name))
          : [join(path, entry.name)],
      ),
    )
  ).flat();
};
interface Sample {
  readonly name: string;
  readonly backend: string;
  readonly repetition: number;
  readonly warmup: boolean;
  readonly wallMs: number;
  readonly outcome: string;
  readonly log: string;
  readonly result?: unknown;
  readonly witness?: readonly unknown[];
}
const samples: Sample[] = [];
const runApalache = async (
  input: Case,
  folder: string,
  length = input.depth,
  initial = "Init",
) => {
  await mkdir(folder, { recursive: true });
  const sourcePath = resolve(
    join(folder, input.family === "ring" ? "Ring.tla" : "Functions.tla"),
  );
  const configuration = resolve(join(folder, "Spec.cfg"));
  await writeFile(sourcePath, input.source, { flag: "wx" });
  await writeFile(
    configuration,
    `INIT ${initial}\nNEXT Next\nINVARIANT Safe\n`,
    { flag: "wx" },
  );
  const args = [
    "-Xmx4096m",
    "-jar",
    resolve(jar),
    `--config-file=${configFile}`,
    `--out-dir=${resolve(join(folder, "output"))}`,
    "check",
    `--config=${configuration}`,
    `--length=${length}`,
    "--algo=incremental",
    "--smt-solver=z3",
    "--smt-encoding=oopsla19",
    sourcePath,
  ];
  await writeFile(
    join(folder, "command.json"),
    JSON.stringify({ executable: java, args }),
    { flag: "wx" },
  );
  const started = performance.now();
  const execution = spawnSync(java, args, {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  if (execution.error) throw execution.error;
  const log = execution.stdout + execution.stderr;
  const outcome = classify(execution.status, log);
  const wallMs = performance.now() - started;
  await writeFile(join(folder, "process.log"), log, { flag: "wx" });
  assert.notEqual(
    outcome,
    "error",
    `${input.name}: Apalache exit ${execution.status}; ${log}`,
  );
  if (outcome === "bounded-no-counterexample")
    assert.ok(
      log.includes(
        `Checker reports no error up to computation length ${length}`,
      ),
    );
  const tracePath =
    outcome === "counterexample"
      ? (await files(join(folder, "output"))).find((path) =>
          path.endsWith("violation1.itf.json"),
        )
      : undefined;
  assert.ok(
    outcome !== "counterexample" || tracePath,
    "Missing counterexample artifact",
  );
  const witness = tracePath
    ? itfStates(JSON.parse(await readFile(tracePath, "utf8")), input.family)
    : undefined;
  if (witness) validateWitness(input, witness);
  return {
    outcome,
    wallMs,
    log: `${folder.slice(directory.length + 1)}/process.log`,
    ...(witness ? { witness } : {}),
  };
};
try {
  for (const input of inputs) {
    const modelDirectory = join(directory, input.name);
    await mkdir(modelDirectory);
    const inputPath = resolve(join(modelDirectory, "input.json"));
    await writeFile(inputPath, JSON.stringify(input), { flag: "wx" });
    const request = `${JSON.stringify({ version: 1, request: { kind: "check-safety", program: input.program, bounds: input.bounds } })}\n`;
    for (let repetition = 0; repetition < 4; repetition++) {
      const order =
        repetition % 2 === 0
          ? ["apalache", "rtlc-source", "rtlc-ir"]
          : ["rtlc-ir", "rtlc-source", "apalache"];
      for (const backend of order) {
        const folder = join(modelDirectory, `${repetition}-${backend}`);
        if (backend === "apalache") {
          const sample = await runApalache(input, folder);
          assert.equal(
            sample.outcome,
            input.expected === "invariant-violation"
              ? "counterexample"
              : "bounded-no-counterexample",
            input.name,
          );
          samples.push({
            name: input.name,
            backend,
            repetition,
            warmup: repetition === 0,
            ...sample,
          });
        } else {
          await mkdir(folder);
          const command =
            backend === "rtlc-source"
              ? process.execPath
              : "crates/idealis-core/target/release/idealis-core";
          const args =
            backend === "rtlc-source"
              ? [
                  "dist/evals/experiments/apalache-comparison/source-worker.js",
                  inputPath,
                ]
              : [];
          const started = performance.now();
          const execution = spawnSync(command, args, {
            encoding: "utf8",
            input: backend === "rtlc-ir" ? request : "",
            maxBuffer: 16 * 1024 * 1024,
          });
          if (execution.error) throw execution.error;
          assert.equal(execution.status, 0, execution.stderr);
          const message: unknown = JSON.parse(execution.stdout);
          assert.ok(isRecord(message) && message.kind === "safety");
          const result = decodeSafetyResult(message.result);
          const wallMs = performance.now() - started;
          await writeFile(
            join(folder, "process.log"),
            execution.stdout + execution.stderr,
            { flag: "wx" },
          );
          assert.equal(result.status, input.expected, input.name);
          if (result.status === "bounded") assert.equal(result.reason, "depth");
          if (result.status === "complete")
            assert.equal(result.states, input.family === "ring" ? 16 : 256);
          const witness = result.violation?.trace.map((step) => {
            assert.equal(step.values.length, 1);
            const value = step.values[0];
            assert.ok(value);
            return fromPortableValue(value);
          });
          if (witness) validateWitness(input, witness);
          samples.push({
            name: input.name,
            backend,
            repetition,
            warmup: repetition === 0,
            wallMs,
            outcome: result.status,
            result,
            log: `${folder.slice(directory.length + 1)}/process.log`,
            ...(witness ? { witness } : {}),
          });
        }
      }
    }
    console.log(`${input.name}: all backends and witnesses checked`);
  }
  const ring = inputs.find((input) => input.name === "ring-safe-15");
  assert.ok(ring);
  const induction = [
    {
      obligation: "Init => Safe",
      ...(await runApalache(
        ring,
        join(directory, "induction-base"),
        0,
        "Init",
      )),
    },
    {
      obligation: "Safe /\\ Next => Safe'",
      ...(await runApalache(
        ring,
        join(directory, "induction-step"),
        1,
        "Safe",
      )),
    },
  ];
  for (const obligation of induction)
    assert.equal(obligation.outcome, "bounded-no-counterexample");
  assert.equal(samples.length, 72);
  for (const entry of [...sources, ...binaries])
    assert.equal(digest(await readFile(entry.path)), entry.sha256, entry.path);
  await writeFile(
    join(directory, "samples.json"),
    JSON.stringify(samples, null, 2),
    { flag: "wx" },
  );
  const summary = inputs.map((input) => ({
    name: input.name,
    backends: ["apalache", "rtlc-source", "rtlc-ir"].map((backend) => {
      const measured = samples.filter(
        (sample) =>
          sample.name === input.name &&
          sample.backend === backend &&
          !sample.warmup,
      );
      const times = measured
        .map((sample) => sample.wallMs)
        .sort((left, right) => left - right);
      assert.equal(times.length, 3);
      return {
        backend,
        medianMs: times[1],
        minMs: times[0],
        maxMs: times[2],
        outcome: measured[0]?.outcome,
      };
    }),
  }));
  const evidence = (await files(directory))
    .map((path) => path.slice(directory.length + 1))
    .sort();
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      {
        status: "complete",
        sourceVersion,
        summary,
        induction,
        measuredSamples: 54,
        warmups: 18,
        files: await Promise.all(
          evidence.map(async (path) => ({
            path,
            sha256: digest(await readFile(join(directory, path))),
          })),
        ),
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "apalache-comparison", status: "complete", sourceVersion })}\n`,
  );
  console.log(
    "54 measurements, 18 warmups and two induction obligations completed",
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", sourceVersion, message: String(error), samples },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "apalache-comparison", status: "error", sourceVersion })}\n`,
  );
  throw error;
}
