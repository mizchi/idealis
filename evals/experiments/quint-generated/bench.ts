import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { arch, cpus, platform } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { digest } from "../usable/provenance.js";
import { names } from "./cases.js";

const directory = process.argv[2];
const mode = process.argv[3];
assert.ok(directory && (mode === undefined || mode === "--replay"));
const expected = names
  .flatMap((name) => [
    `quint-parse/${name}`,
    ...[1, 2].flatMap((version) =>
      ["compile", "check", "end-to-end"].map(
        (stage) => `quint-${stage}/${name}/v${version}`,
      ),
    ),
  ])
  .sort();
const filesIn = async (directory: string): Promise<string[]> =>
  (
    await Promise.all(
      (await readdir(directory, { withFileTypes: true })).map(async (entry) => {
        assert.ok(
          !entry.isSymbolicLink(),
          "Benchmark evidence must not be a symlink",
        );
        const path = join(directory, entry.name);
        return entry.isDirectory() ? filesIn(path) : [path];
      }),
    )
  )
    .flat()
    .sort();
const json = async (path: string): Promise<unknown> =>
  JSON.parse(await readFile(path, "utf8"));
const snapshot = async () => {
  const base = await fingerprint();
  const paths = execFileSync("git", [
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
    "--",
    "evals/experiments/quint-generated",
    "fixtures/quint-generated",
    "evals/plans/quint-sharing-regression.md",
  ])
    .toString()
    .split("\0")
    .filter(Boolean);
  const sources = [
    ...base.sources,
    ...(await Promise.all(
      paths.map(async (path) => ({
        path,
        sha256: digest(await readFile(path)),
      })),
    )),
  ].sort((left, right) => left.path.localeCompare(right.path));
  return { sources, sourceVersion: digest(JSON.stringify(sources)) };
};
const summarize = async () => {
  const benchmarks = [];
  for (const path of await filesIn(join(directory, "criterion"))) {
    if (!path.endsWith("/new/benchmark.json")) continue;
    const benchmark = await json(path);
    const estimates = await json(join(dirname(path), "estimates.json"));
    const sample = await json(join(dirname(path), "sample.json"));
    assert.ok(isRecord(benchmark) && typeof benchmark.full_id === "string");
    assert.ok(isRecord(estimates) && isRecord(estimates.median));
    const interval = estimates.median.confidence_interval;
    assert.ok(isRecord(interval) && interval.confidence_level === 0.95);
    for (const value of [
      estimates.median.point_estimate,
      interval.lower_bound,
      interval.upper_bound,
    ])
      assert.ok(
        typeof value === "number" && Number.isFinite(value) && value > 0,
      );
    assert.ok(
      isRecord(sample) &&
        Array.isArray(sample.iters) &&
        Array.isArray(sample.times),
    );
    assert.equal(sample.iters.length, 20);
    assert.equal(sample.times.length, 20);
    for (const value of [...sample.iters, ...sample.times])
      assert.ok(
        typeof value === "number" && Number.isFinite(value) && value > 0,
      );
    benchmarks.push({
      id: benchmark.full_id,
      medianNs: estimates.median.point_estimate,
      lowerNs: interval.lower_bound,
      upperNs: interval.upper_bound,
      samples: sample.times.length,
    });
  }
  benchmarks.sort((left, right) => left.id.localeCompare(right.id));
  assert.deepEqual(benchmarks.map((entry) => entry.id).sort(), expected);
  const programs: unknown[] = (
    await readFile(join(directory, "stdout.log"), "utf8")
  )
    .split("\n")
    .filter((line) => line.startsWith("IDEALIS_QUINT_PROGRAM "))
    .map((line) => JSON.parse(line.slice("IDEALIS_QUINT_PROGRAM ".length)));
  assert.deepEqual(
    programs
      .map((entry) => {
        assert.ok(isRecord(entry));
        for (const key of ["version", "irBytes", "states", "generated", "work"])
          assert.ok(
            typeof entry[key] === "number" &&
              Number.isSafeInteger(entry[key]) &&
              entry[key] > 0,
          );
        return `${String(entry.name)}/v${String(entry.version)}`;
      })
      .sort(),
    names.flatMap((name) => [`${name}/v1`, `${name}/v2`]).sort(),
  );
  return { unit: "nanoseconds", benchmarks, programs };
};

if (mode === "--replay") {
  const result = await json(join(directory, "result.json"));
  const manifest = await json(join(directory, "manifest.json"));
  assert.ok(
    isRecord(result) &&
      result.status === "complete" &&
      Array.isArray(result.files),
  );
  assert.ok(isRecord(manifest));
  assert.equal(result.sourceVersion, manifest.sourceVersion);
  assert.equal(manifest.sourceVersion, (await snapshot()).sourceVersion);
  const expectedFiles = result.files.map((entry: unknown) => {
    assert.ok(
      isRecord(entry) &&
        typeof entry.path === "string" &&
        typeof entry.sha256 === "string",
    );
    assert.ok(
      !entry.path.startsWith("/") && !entry.path.split("/").includes(".."),
    );
    return { path: entry.path, sha256: entry.sha256 };
  });
  assert.deepEqual(
    (await filesIn(directory))
      .map((path) => relative(directory, path))
      .filter((path) => path !== "result.json")
      .sort(),
    expectedFiles.map((entry) => entry.path).sort(),
  );
  for (const entry of expectedFiles)
    assert.equal(
      digest(await readFile(join(directory, entry.path))),
      entry.sha256,
      entry.path,
    );
  assert.deepEqual(await summarize(), result.summary);
  console.log(`Replayed ${expected.length} Quint Criterion measurements`);
} else {
  await mkdir(dirname(directory), { recursive: true });
  await mkdir(directory);
  try {
    const source = await snapshot();
    const buildCommand = [
      "bench",
      "--locked",
      "--manifest-path",
      "crates/rtla-compiler/Cargo.toml",
      "--bench",
      "quint_generated",
      "--no-run",
      "--message-format=json",
    ];
    const build = spawnSync("cargo", buildCommand, {
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    });
    await writeFile(join(directory, "build-stdout.log"), build.stdout ?? "", {
      flag: "wx",
    });
    await writeFile(join(directory, "build-stderr.log"), build.stderr ?? "", {
      flag: "wx",
    });
    if (build.error) throw build.error;
    assert.equal(build.status, 0, build.stderr);
    const artifacts = (build.stdout ?? "")
      .split("\n")
      .filter(Boolean)
      .map((line): unknown => JSON.parse(line))
      .filter(
        (entry) =>
          isRecord(entry) &&
          entry.reason === "compiler-artifact" &&
          isRecord(entry.target) &&
          entry.target.name === "quint_generated" &&
          typeof entry.executable === "string",
      );
    assert.equal(artifacts.length, 1);
    const artifact = artifacts[0];
    assert.ok(isRecord(artifact) && typeof artifact.executable === "string");
    const binarySha256 = digest(await readFile(artifact.executable));
    const command = [artifact.executable, "--bench", "--noplot"];
    await writeFile(
      join(directory, "manifest.json"),
      JSON.stringify(
        {
          plan: "quint-sharing-regression",
          ...source,
          commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
          node: process.version,
          platform: platform(),
          arch: arch(),
          cpu: cpus()[0]?.model,
          rustc: execFileSync("rustc", ["-Vv"]).toString(),
          buildCommand: ["cargo", ...buildCommand],
          command,
          binary: {
            path: artifact.executable,
            sha256: binarySha256,
            profile: artifact.profile,
          },
          criterion: {
            samples: 20,
            warmUpMs: 500,
            measurementMs: 1000,
            confidence: 0.95,
          },
          stages: {
            parse: "module and config",
            compile: "preparsed AST",
            check: "compiled IR",
            "end-to-end": "parse, compile and check",
          },
          includes: ["result destruction"],
          excludes: ["Quint generation", "JVM startup", "JSONL transport"],
          generation: await json(
            "fixtures/quint-generated/generated/manifest.json",
          ),
        },
        null,
        2,
      ) + "\n",
      { flag: "wx" },
    );
    const run = spawnSync(artifact.executable, command.slice(1), {
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
      env: {
        ...process.env,
        IDEALIS_QUINT_BENCH_OUTPUT: resolve(directory, "criterion"),
      },
    });
    await writeFile(join(directory, "stdout.log"), run.stdout ?? "", {
      flag: "wx",
    });
    await writeFile(join(directory, "stderr.log"), run.stderr ?? "", {
      flag: "wx",
    });
    if (run.error) throw run.error;
    assert.equal(run.status, 0, run.stderr);
    const summary = await summarize();
    assert.equal(digest(await readFile(artifact.executable)), binarySha256);
    assert.equal((await snapshot()).sourceVersion, source.sourceVersion);
    const files = await Promise.all(
      (await filesIn(directory)).map(async (path) => ({
        path: relative(directory, path),
        sha256: digest(await readFile(path)),
      })),
    );
    await writeFile(
      join(directory, "result.json"),
      JSON.stringify(
        {
          status: "complete",
          sourceVersion: source.sourceVersion,
          summary,
          files,
        },
        null,
        2,
      ) + "\n",
      { flag: "wx" },
    );
    console.log(
      `Saved ${summary.benchmarks.length} Quint Criterion measurements to ${directory}`,
    );
  } catch (error) {
    await writeFile(
      join(directory, "error.json"),
      JSON.stringify({ error: String(error) }, null, 2) + "\n",
      { flag: "wx" },
    );
    throw error;
  }
}
