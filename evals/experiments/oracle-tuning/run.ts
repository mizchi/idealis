import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, totalmem } from "node:os";
import { dirname, join, resolve } from "node:path";
import { catalog } from "../workspace-oracle/catalog.js";
import { costWorkerFiles } from "../workspace-oracle/worker-files.js";

const digest = (bytes: Uint8Array | string) =>
  createHash("sha256").update(bytes).digest("hex");
const save = async (path: string, value: unknown) =>
  writeFile(path, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" });
interface Frozen {
  readonly files: readonly { readonly path: string; readonly sha256: string }[];
}
const verify = async (directory: string) => {
  const manifest: Frozen = JSON.parse(
    await readFile(join(directory, "manifest.json"), "utf8"),
  );
  for (const file of manifest.files)
    assert.equal(
      digest(await readFile(join(directory, file.path))),
      file.sha256,
      file.path,
    );
};
const command = process.argv[2];
const first = process.argv[3];
assert.ok(first);
if (command === "freeze") {
  await mkdir(dirname(first), { recursive: true });
  await mkdir(first);
  const files = await Promise.all(
    costWorkerFiles.map(async (path) => {
      const bytes = await readFile(path);
      await mkdir(dirname(join(first, path)), { recursive: true });
      await writeFile(join(first, path), bytes, { flag: "wx" });
      return { path, sha256: digest(bytes) };
    }),
  );
  for (const [path, target] of [
    [
      "evals/experiments/workspace-oracle/rust/target/release/workspace-oracle",
      "runner",
    ],
    ["crates/roracle/target/release/roracle", "oracle"],
  ]) {
    assert.ok(path && target);
    const bytes = await readFile(path);
    await writeFile(join(first, target), bytes, { flag: "wx", mode: 0o755 });
    files.push({ path: target, sha256: digest(bytes) });
  }
  await save(join(first, "package.json"), { type: "module" });
  const bootstrap =
    'import { fileURLToPath } from "node:url"; process.chdir(fileURLToPath(new URL(".", import.meta.url))); await import("./dist/evals/experiments/workspace-oracle/cost-worker.js");\n';
  await writeFile(join(first, "worker.mjs"), bootstrap, { flag: "wx" });
  files.push({ path: "worker.mjs", sha256: digest(bootstrap) });
  const cases = catalog()
    .filter((suite) => suite.id === "rtla" || suite.id === "idealis-core")
    .map((suite) => {
      const model = suite.cases[0];
      assert.ok(model);
      const reply: unknown = JSON.parse(
        execFileSync(suite.binary, [], {
          input: `${JSON.stringify(model.request)}\n`,
        }).toString(),
      );
      return {
        id: suite.id,
        request: { actual: reply, expected: reply },
        requirements: [{ pointer: "/status", value: "matched" }],
      };
    });
  await save(join(first, "cases.json"), cases);
  files.push({
    path: "cases.json",
    sha256: digest(await readFile(join(first, "cases.json"))),
  });
  const sources = execFileSync("git", [
    "ls-files",
    "--cached",
    "--others",
    "--exclude-standard",
    "crates/roracle",
    "evals/experiments/workspace-oracle",
    "tests/workspace-oracle",
    "fixtures/workspace-oracle",
    "evals/experiments/roracle-native/client.ts",
    "evals/experiments/roracle-native/canonical.ts",
    "evals/experiments/oracle-tuning",
    "tests/oracle-tuning",
    "evals/plans/0054-oracle-bench-tuning.md",
  ])
    .toString()
    .trim()
    .split("\n");
  const fingerprints = await Promise.all(
    [...new Set(sources)]
      .sort()
      .map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  );
  await save(join(first, "manifest.json"), {
    plan: "0054",
    files,
    sources: fingerprints,
    revision: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    node: process.version,
    nodePath: process.execPath,
    nodeSha256: digest(await readFile(process.execPath)),
    cpu: cpus()[0]?.model,
    memoryBytes: totalmem(),
  });
  console.log(`frozen ${files.length} artifacts: ${first}`);
} else if (
  command === "measure" ||
  command === "profile" ||
  command === "compare"
) {
  const transport = process.argv[4];
  const after = process.argv[5];
  const output = process.argv[6];
  assert.ok(transport && after && output);
  await Promise.all([first, transport, after].map(verify));
  const cases = JSON.parse(await readFile(join(first, "cases.json"), "utf8"));
  for (const directory of [transport, after])
    assert.deepEqual(
      JSON.parse(await readFile(join(directory, "cases.json"), "utf8")),
      cases,
    );
  await mkdir(dirname(output), { recursive: true });
  await mkdir(output);
  await save(join(output, "manifest.json"), {
    plan: command === "compare" ? "0055" : "0054",
    snapshots: await Promise.all(
      [first, transport, after].map(async (path) => ({
        path: resolve(path),
        sha256: digest(await readFile(join(path, "manifest.json"))),
      })),
    ),
    node: process.version,
    cpu: cpus()[0]?.model,
    scope:
      "reused Node roundtrip including parent serialization, IPC and whole oracle comparison",
  });
  const stages =
    command === "profile"
      ? ["before"]
      : command === "compare"
        ? ["before", "wire", "combined", "combined", "wire", "before"]
        : ["before", "transport", "bridge", "bridge", "transport", "before"];
  const records = [];
  for (const [index, stage] of stages.entries()) {
    const root =
      stage === "bridge" || stage === "combined"
        ? after
        : stage === "wire"
          ? transport
          : first;
    const runner = resolve(
      command === "compare" || stage === "before" ? first : transport,
      "runner",
    );
    const name = `${index}-${stage}`;
    const input = {
      version: 1,
      name,
      mode: "reused",
      baseline: {
        path: process.execPath,
        args: [resolve(root, "worker.mjs"), "legacy"],
      },
      candidate: {
        path: process.execPath,
        args: [resolve(root, "worker.mjs"), "oracle"],
      },
      cases,
      config: {
        baseline: "baseline",
        scope:
          "Node reused roundtrip, request serialization and full comparison",
        samples: 20,
        warmup: 75,
        iterations: 25,
        max_evaluations: 10000,
      },
    };
    await save(join(output, `${name}.json`), input);
    const args = [resolve(output, `${name}.json`), resolve(output, name)];
    if (command === "profile")
      execFileSync(
        "samply",
        [
          "record",
          "-s",
          "--main-thread-only",
          "--unstable-presymbolicate",
          "-o",
          resolve(output, "profile.json.gz"),
          "--",
          runner,
          ...args,
        ],
        {
          stdio: "inherit",
          env: { ...process.env, DYLD_LIBRARY_PATH: "/usr/lib" },
        },
      );
    else execFileSync(runner, args, { stdio: "inherit" });
    const report = JSON.parse(
      await readFile(join(output, name, "bench.json"), "utf8"),
    );
    assert.equal(report.status, "matched");
    assert.equal(report.evaluations, report.planned_evaluations);
    records.push({ stage, index, cases: report.cases });
  }
  await save(join(output, "result.json"), {
    status: "complete",
    measurement: command !== "profile",
    records,
  });
} else
  throw new Error(
    "Use freeze <new-dir> or measure/profile/compare <before> <transport> <after> <new-run>",
  );
