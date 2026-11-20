import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { cpus, totalmem } from "node:os";
import { dirname, join, resolve } from "node:path";
import { catalog, type Fixture } from "./catalog.js";
import { assertOracle } from "./node.js";

interface Snapshot {
  readonly version: number;
  readonly revision: string;
  readonly sources: readonly {
    readonly path: string;
    readonly sha256: string;
  }[];
  readonly binaries: readonly {
    readonly id: string;
    readonly sha256: string;
    readonly cases: readonly Fixture[];
  }[];
}
const digest = (bytes: Uint8Array | string) =>
  createHash("sha256").update(bytes).digest("hex");
const save = async (path: string, value: unknown) =>
  writeFile(path, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" });
const artifacts = async (
  directory: string,
  prefix = "",
): Promise<readonly { path: string; sha256: string }[]> => {
  const entries = await readdir(join(directory, prefix), {
    withFileTypes: true,
  });
  return (
    await Promise.all(
      entries.map(async (entry) => {
        const path = join(prefix, entry.name);
        return entry.isDirectory()
          ? artifacts(directory, path)
          : [{ path, sha256: digest(await readFile(join(directory, path))) }];
      }),
    )
  ).flat();
};
// Frozen snapshots keep the executable under the suite name used when recorded.
const renamedSuites = new Map([
  ["idealis-smt", "rusmt"],
  ["idealis-testing", "idealis-connect"],
]);
const suiteId = (id: string) => renamedSuites.get(id) ?? id;
const readSnapshot = async (directory: string): Promise<Snapshot> => {
  const snapshot: Snapshot = JSON.parse(
    await readFile(join(directory, "manifest.json"), "utf8"),
  );
  assert.equal(snapshot.version, 1);
  assert.ok(snapshot.binaries.length > 0);
  assert.equal(
    new Set(snapshot.binaries.map((entry) => suiteId(entry.id))).size,
    snapshot.binaries.length,
  );
  for (const entry of snapshot.binaries) {
    assert.match(entry.id, /^[a-z][a-z0-9-]*$/);
    assert.equal(
      digest(await readFile(join(directory, entry.id))),
      entry.sha256,
      `${entry.id}: snapshot binary changed`,
    );
    assert.ok(entry.cases.length > 0);
  }
  for (const source of snapshot.sources) {
    assert.ok(
      !source.path.startsWith("/") && !source.path.split("/").includes(".."),
    );
    assert.equal(
      digest(await readFile(join(directory, "source", source.path))),
      source.sha256,
      source.path,
    );
  }
  return snapshot;
};

const command = process.argv[2];
const first = process.argv[3];
assert.ok(first);
if (command === "snapshot") {
  await mkdir(dirname(first), { recursive: true });
  await mkdir(first);
  const paths = execFileSync("git", [
    "ls-files",
    "--cached",
    "--others",
    "--exclude-standard",
    "crates",
    "evals/experiments/workspace-oracle",
    "tests/workspace-oracle",
    "fixtures/workspace-oracle",
    "evals/plans/0052-workspace-oracle.md",
  ])
    .toString()
    .trim()
    .split("\n");
  const sources = await Promise.all(
    paths.map(async (path) => {
      const bytes = await readFile(path);
      const target = join(first, "source", path);
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, bytes, { flag: "wx" });
      return { path, sha256: digest(bytes) };
    }),
  );
  const binaries = await Promise.all(
    catalog().map(async (suite) => {
      const bytes = await readFile(suite.binary);
      await writeFile(join(first, suite.id), bytes, {
        flag: "wx",
        mode: 0o755,
      });
      return { id: suite.id, sha256: digest(bytes), cases: suite.cases };
    }),
  );
  for (const source of sources)
    assert.equal(
      digest(await readFile(source.path)),
      source.sha256,
      `${source.path}: changed during snapshot`,
    );
  await save(join(first, "manifest.json"), {
    version: 1,
    revision: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    sources,
    binaries,
    cpu: cpus()[0]?.model,
    memoryBytes: totalmem(),
    node: process.version,
    rust: execFileSync("rustc", ["--version"]).toString().trim(),
  });
  console.log(
    `saved ${binaries.length} executables and ${sources.length} sources: ${first}`,
  );
} else if (command === "run") {
  const second = process.argv[4];
  const output = process.argv[5];
  assert.ok(second && output);
  const before = await readSnapshot(first);
  const after = await readSnapshot(second);
  assertOracle(
    after.binaries.map((entry) => ({
      id: suiteId(entry.id),
      cases: entry.cases,
    })),
    before.binaries.map((entry) => ({
      id: suiteId(entry.id),
      cases: entry.cases,
    })),
    "baseline/candidate suite identities and complete requests",
  );
  await mkdir(dirname(output), { recursive: true });
  await mkdir(output);
  const runner = resolve(
    "evals/experiments/workspace-oracle/rust/target/release/workspace-oracle",
  );
  await save(join(output, "manifest.json"), {
    plan: "0052",
    baseline: resolve(first),
    candidate: resolve(second),
    snapshots: [
      digest(await readFile(join(first, "manifest.json"))),
      digest(await readFile(join(second, "manifest.json"))),
    ],
    runner,
    runnerSha256: digest(await readFile(runner)),
    cpu: cpus()[0]?.model,
    memoryBytes: totalmem(),
  });
  const records = [];
  try {
    for (const [index, suite] of before.binaries.entries()) {
      const candidate = after.binaries[index];
      assert.ok(candidate);
      for (const mode of ["cold", "reused"]) {
        const name = `${suiteId(suite.id)}-${mode}`;
        const input = {
          version: 1,
          name,
          mode,
          baseline: { path: resolve(first, suite.id), args: [] },
          candidate: { path: resolve(second, candidate.id), args: [] },
          cases: suite.cases,
          config: {
            baseline: "baseline",
            scope:
              mode === "cold"
                ? "fresh process, JSONL transport, request and response serialization"
                : "reused independent JSONL processes, transport, request and response serialization",
            samples: mode === "cold" ? 10 : 20,
            warmup: mode === "cold" ? 2 : 75,
            iterations: mode === "cold" ? 1 : 25,
            max_evaluations: 10000,
          },
        };
        const path = join(output, `${name}.json`);
        await save(path, input);
        execFileSync(runner, [resolve(path), resolve(output, name)], {
          stdio: "inherit",
        });
        const report = JSON.parse(
          await readFile(join(output, name, "bench.json"), "utf8"),
        );
        assert.equal(report.status, "matched");
        assert.equal(report.evaluations, report.planned_evaluations);
        records.push({
          name,
          cases: report.cases.map(
            (entry: { id: string; measurements: unknown }) => ({
              id: entry.id,
              measurements: entry.measurements,
            }),
          ),
        });
      }
    }
    await save(join(output, "result.json"), {
      status: "complete",
      records,
      files: await artifacts(output),
    });
    console.log(`${records.length} groups matched and measured`);
  } catch (error) {
    await save(join(output, "error.json"), {
      status: "error",
      message: String(error),
      completed: records,
    });
    throw error;
  }
} else if (command === "replay") {
  const output = process.argv[4];
  assert.ok(output);
  const manifest = JSON.parse(
    await readFile(join(first, "manifest.json"), "utf8"),
  );
  assert.equal(digest(await readFile(manifest.runner)), manifest.runnerSha256);
  for (const [index, path] of [
    manifest.baseline,
    manifest.candidate,
  ].entries()) {
    await readSnapshot(path);
    assert.equal(
      digest(await readFile(join(path, "manifest.json"))),
      manifest.snapshots[index],
    );
  }
  const result = JSON.parse(await readFile(join(first, "result.json"), "utf8"));
  assert.equal(result.status, "complete");
  for (const file of result.files) {
    assert.ok(
      typeof file.path === "string" &&
        !file.path.startsWith("/") &&
        !file.path.split("/").includes(".."),
    );
    assert.equal(
      digest(await readFile(join(first, file.path))),
      file.sha256,
      file.path,
    );
  }
  await mkdir(dirname(output), { recursive: true });
  await mkdir(output);
  for (const entry of result.records) {
    execFileSync(
      manifest.runner,
      [
        resolve(first, `${entry.name}.json`),
        resolve(output, entry.name),
        resolve(first, entry.name, "check.json"),
      ],
      { stdio: "inherit" },
    );
  }
  await save(join(output, "result.json"), {
    status: "complete",
    replayed: result.records.length,
    parent: first,
  });
  console.log(
    `${result.records.length} groups replayed against recorded oracles`,
  );
} else
  throw new Error(
    "Use snapshot <new-dir>, run <before> <after> <new-run>, replay <run> <new-replay>",
  );
