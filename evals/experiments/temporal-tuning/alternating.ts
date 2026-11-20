import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  copyFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { cpus, loadavg } from "node:os";
import { join, resolve } from "node:path";
import { fingerprint } from "../rust-safety/provenance.js";
import { digest } from "../usable/provenance.js";

interface Artifact {
  readonly executable?: string | null;
  readonly target?: { readonly name: string };
}
interface Estimate {
  readonly mean: {
    readonly point_estimate: number;
    readonly confidence_interval: unknown;
  };
}
interface Binary {
  readonly suite: string;
  readonly variant: string;
  readonly path: string;
  readonly sha256: string;
}
interface Benchmark {
  readonly full_id: string;
  readonly directory_name: string;
}
const directory = process.argv[2];
assert.ok(directory);
await mkdir(directory);
const scratch = resolve(".artifacts/0049-alternating");
await mkdir(scratch);
const base = await fingerprint();
const sources = [
  ...base.sources,
  ...(await Promise.all(
    [
      "evals/plans/0049-temporal-tuning.md",
      "evals/experiments/temporal-tuning/alternating.ts",
    ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  )),
];
const revision = execFileSync("git", ["rev-parse", "0e9212ac^{commit}"])
  .toString()
  .trim();
const archive = execFileSync(
  "git",
  [
    "archive",
    revision,
    "--",
    "crates/rtla",
    "crates/rtlc",
    "crates/rtla-compiler",
  ],
  { maxBuffer: 32 * 1024 * 1024 },
);
await writeFile(join(directory, "source.tar"), archive, { flag: "wx" });
execFileSync("tar", ["-xf", resolve(directory, "source.tar"), "-C", scratch]);
const suites = [
  {
    name: "compiler",
    crate: "rtla-compiler",
    bench: "compatibility",
    reference: "evals/runs/0049-criterion-before",
  },
  {
    name: "controls",
    crate: "rtlc",
    bench: "safety",
    reference: "evals/runs/0049-controls-before",
  },
];
const binaries: Binary[] = [];
for (const suite of suites)
  for (const variant of ["before", "after"]) {
    const manifest =
      variant === "before"
        ? join(scratch, "crates", suite.crate, "Cargo.toml")
        : `crates/${suite.crate}/Cargo.toml`;
    const log = execFileSync(
      "cargo",
      [
        "bench",
        "--locked",
        "--no-run",
        "--manifest-path",
        manifest,
        "--bench",
        suite.bench,
        "--message-format=json",
      ],
      { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
    );
    await writeFile(
      join(directory, `${suite.name}-${variant}-build.jsonl`),
      log,
      { flag: "wx" },
    );
    const artifacts: Artifact[] = log
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    const artifact = artifacts.findLast(
      (row) => row.target?.name === suite.bench && row.executable,
    );
    assert.ok(artifact?.executable);
    const path = join(scratch, `${suite.name}-${variant}`);
    await copyFile(artifact.executable, path);
    binaries.push({
      suite: suite.name,
      variant,
      path,
      sha256: digest(await readFile(path)),
    });
  }
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "0049-temporal-tuning",
      revision,
      sources,
      binaries,
      rust: execFileSync("rustc", ["--version"]).toString().trim(),
      cpu: cpus()[0]?.model,
      loadAverage: loadavg(),
      dedicatedHost: false,
      conditions: {
        order: ["before-1", "after-1", "after-2", "before-2"],
        sampleSize: 20,
        warmupSeconds: 0.5,
        measurementSeconds: 1,
        statistic:
          "arithmetic mean of two Criterion means for each variant; no pooled confidence interval",
      },
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const comparisons = [];
try {
  for (const suite of suites) {
    const reference = JSON.parse(
      await readFile(join(suite.reference, "result.json"), "utf8"),
    );
    assert.equal(reference.status, "complete");
    for (const original of reference.estimates) {
      const benchmark: Benchmark = JSON.parse(
        await readFile(
          join(
            suite.reference,
            "criterion",
            original.benchmark,
            reference.stage,
            "benchmark.json",
          ),
          "utf8",
        ),
      );
      const measurements = [];
      for (const stage of ["before-1", "after-1", "after-2", "before-2"]) {
        const binary: Binary | undefined = binaries.find(
          (row) => row.suite === suite.name && stage.startsWith(row.variant),
        );
        assert.ok(binary);
        const criterionHome = resolve(directory, "criterion", suite.name);
        const args: string[] = [
          "--bench",
          "--noplot",
          "--exact",
          benchmark.full_id,
          "--save-baseline",
          stage,
        ];
        const child = spawnSync(binary.path, args, {
          env: { ...process.env, CRITERION_HOME: criterionHome },
          encoding: "utf8",
          maxBuffer: 16 * 1024 * 1024,
        });
        const prefix = `${suite.name}-${benchmark.directory_name.replaceAll("/", "_")}-${stage}`;
        await writeFile(
          join(directory, `${prefix}.log`),
          child.stdout + child.stderr,
          { flag: "wx" },
        );
        assert.equal(child.error, undefined);
        assert.equal(child.status, 0, child.stderr);
        const estimate: Estimate = JSON.parse(
          await readFile(
            join(
              criterionHome,
              benchmark.directory_name,
              stage,
              "estimates.json",
            ),
            "utf8",
          ),
        );
        const sample = JSON.parse(
          await readFile(
            join(criterionHome, benchmark.directory_name, stage, "sample.json"),
            "utf8",
          ),
        );
        assert.equal(sample.iters.length, 20);
        assert.ok(
          Number.isFinite(estimate.mean.point_estimate) &&
            estimate.mean.point_estimate > 0,
        );
        measurements.push({
          stage,
          meanNs: estimate.mean.point_estimate,
          confidence: estimate.mean.confidence_interval,
          loadAverage: loadavg(),
        });
      }
      const before =
        measurements
          .filter((row) => row.stage.startsWith("before"))
          .reduce((total, row) => total + row.meanNs, 0) / 2;
      const after =
        measurements
          .filter((row) => row.stage.startsWith("after"))
          .reduce((total, row) => total + row.meanNs, 0) / 2;
      comparisons.push({
        suite: suite.name,
        benchmark: benchmark.full_id,
        beforeNs: before,
        afterNs: after,
        speedup: before / after,
        regression: after > before * 1.03,
        measurements,
      });
      console.log(
        `${suite.name}/${benchmark.full_id}: ${(before / after).toFixed(3)}x`,
      );
    }
  }
  for (const file of sources)
    assert.equal(digest(await readFile(file.path)), file.sha256, file.path);
  for (const binary of binaries)
    assert.equal(digest(await readFile(binary.path)), binary.sha256);
  assert.equal(comparisons.length, 41);
  const files: { path: string; sha256: string }[] = [];
  const collect = async (relative = ""): Promise<void> => {
    for (const entry of await readdir(join(directory, relative), {
      withFileTypes: true,
    })) {
      const path = join(relative, entry.name);
      if (entry.isDirectory()) await collect(path);
      else
        files.push({
          path,
          sha256: digest(await readFile(join(directory, path))),
        });
    }
  };
  await collect();
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      {
        status: "complete",
        sourceVersion: digest(JSON.stringify(sources)),
        comparisons,
        files,
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", message: String(error), comparisons },
      null,
      2,
    ),
    { flag: "wx" },
  );
  throw error;
}
