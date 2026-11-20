import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { order, summarize } from "../checker-benchmark/protocol.js";
import { environment } from "../checker-benchmark/provenance.js";
import { fingerprint } from "../parallel-safety/provenance.js";
import { typeScriptSources } from "../source-files.js";
import { digest } from "../usable/provenance.js";
import { loadDataset } from "./dataset.js";
import { routes } from "./protocol.js";

const java = process.argv[2];
const jar = process.argv[3];
const archive = process.argv[4];
const directory = process.argv[5];
const saved = process.argv[6];
assert.ok(
  java && jar && archive && directory,
  "Pass java, jar, JRE archive, new output directory, optional saved run",
);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory, { recursive: false });
const files: string[] = [];
const write = async (path: string, value: unknown) => {
  await writeFile(
    join(directory, path),
    `${JSON.stringify(value, null, 2)}\n`,
    { flag: "wx" },
  );
  files.push(path);
};
const base = await fingerprint();
const extraPaths = [
  "evals/plans/0018-tlc-throughput.md",
  ...(await typeScriptSources(
    "evals/experiments/tlc-throughput",
    "tests/tlc-throughput",
  )),
];
const sources = [
  ...base.sources,
  ...(await Promise.all(
    extraPaths.map(async (path) => ({
      path,
      sha256: digest(await readFile(path)),
    })),
  )),
].sort((left, right) => left.path.localeCompare(right.path));
const sourceVersion = digest(JSON.stringify(sources));
const samples: {
  route: string;
  iteration: number;
  warmup: boolean;
  wallMs: number;
  workMs: number;
  path: string;
}[] = [];
try {
  const tools = await environment(java, jar, archive);
  const dataset = await loadDataset();
  if (saved) {
    const result = JSON.parse(
      await readFile(join(saved, "result.json"), "utf8"),
    );
    assert.equal(result.status, "complete");
    assert.equal(
      result.sourceVersion,
      sourceVersion,
      "Check out recorded sources",
    );
    for (const file of result.files) {
      assert.ok(
        typeof file.path === "string" &&
          !file.path.includes("..") &&
          !file.path.startsWith("/"),
      );
      assert.equal(digest(await readFile(join(saved, file.path))), file.sha256);
    }
    assert.deepEqual(
      JSON.parse(await readFile(join(saved, "dataset.json"), "utf8")),
      dataset,
    );
    const manifest = JSON.parse(
      await readFile(join(saved, "manifest.json"), "utf8"),
    );
    assert.deepEqual(manifest.environment.binaries, tools.binaries);
    assert.equal(result.samples.length, 16);
    assert.equal(
      new Set(
        result.samples.map(
          (sample: { route: string; iteration: number }) =>
            `${sample.route}/${sample.iteration}`,
        ),
      ).size,
      16,
    );
    assert.deepEqual(
      result.summaries,
      routes.map((route) => ({
        route,
        ...summarize(
          result.samples
            .filter(
              (sample: { route: string; warmup: boolean }) =>
                sample.route === route && !sample.warmup,
            )
            .map((sample: { wallMs: number }) => sample.wallMs),
        ),
      })),
    );
    for (const sample of result.samples) {
      const evidence = JSON.parse(
        await readFile(join(saved, sample.path), "utf8"),
      );
      assert.equal(evidence.exit, 0);
      assert.equal(evidence.wallMs, sample.wallMs);
      assert.equal(evidence.output.completed.length, 88);
      const actual = new Map(
        evidence.output.completed.map(
          (entry: { id: string; result: unknown }) => [entry.id, entry.result],
        ),
      );
      assert.equal(actual.size, 88);
      for (const job of dataset.jobs) {
        const expected = dataset.parentManifest.tasks.find(
          (task) => task.id === job.id,
        );
        assert.ok(expected);
        assert.deepEqual(actual.get(job.id), {
          status: "complete",
          states: expected.payload.states,
          generated: expected.payload.generated,
        });
      }
    }
  }
  await write("manifest.json", {
    plan: "0018-tlc-throughput",
    sources,
    sourceVersion,
    saved: saved ?? null,
    commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    dirtyPaths: execFileSync("git", ["status", "--short"]).toString().trim(),
    startedAt: new Date().toISOString(),
    environment: tools,
    warmup: saved ? 0 : 1,
    measurements: saved ? 1 : 3,
  });
  await write("dataset.json", dataset);
  await mkdir(join(directory, "inputs"));
  for (const job of dataset.jobs) {
    const task = dataset.parentManifest.tasks.find(
      (task) => task.id === job.id,
    );
    const input =
      task && dataset.inputs.find((input) => input.name === task.payload.name);
    const moduleName =
      input && /^---- MODULE (\w+) ----/m.exec(input.source)?.[1];
    assert.ok(input && moduleName);
    await mkdir(join(directory, "inputs", job.id));
    await mkdir(join(directory, "inputs", job.id, "tmp"));
    for (const [suffix, content] of [
      ["tla", input.source],
      ["cfg", input.config],
    ] as const) {
      const path = `inputs/${job.id}/${moduleName}.${suffix}`;
      await writeFile(join(directory, path), content, { flag: "wx" });
      files.push(path);
    }
  }
  for (let iteration = 0; iteration < (saved ? 1 : 4); iteration++) {
    for (const route of order(routes, iteration)) {
      console.log(`Starting ${route}, iteration ${iteration}`);
      const args = [
        resolve("dist/evals/experiments/tlc-throughput/worker.js"),
        route,
        resolve(directory, "dataset.json"),
        resolve(directory, "inputs"),
        resolve(java),
        resolve(jar),
      ];
      const start = performance.now();
      const raw = await new Promise<{
        exit: number | null;
        stdout: string;
        stderr: string;
      }>((resolveResult, reject) => {
        const child = spawn(process.execPath, args, {
          stdio: ["ignore", "pipe", "pipe"],
        });
        let stdout = "";
        let stderr = "";
        child.stdout.on("data", (chunk) => {
          stdout += String(chunk);
        });
        child.stderr.on("data", (chunk) => {
          stderr += String(chunk);
          process.stderr.write(chunk);
        });
        child.once("error", reject);
        child.once("close", (exit) => resolveResult({ exit, stdout, stderr }));
      });
      let output:
        | {
            route: string;
            workMs: number;
            completed: { id: string; result: unknown }[];
          }
        | undefined;
      let wallMs: number;
      try {
        output = JSON.parse(raw.stdout);
        wallMs = performance.now() - start;
      } finally {
        await write(`${route}-${iteration}-raw.json`, raw);
      }
      const path = `${route}-${iteration}.json`;
      await write(path, { wallMs, exit: raw.exit, output });
      assert.equal(raw.exit, 0);
      assert.ok(output && output.route === route);
      assert.equal(output.completed.length, 88);
      const actual = new Map(
        output.completed.map((entry) => [entry.id, entry.result]),
      );
      assert.equal(actual.size, 88);
      for (const job of dataset.jobs) {
        const expected = dataset.parentManifest.tasks.find(
          (task) => task.id === job.id,
        );
        assert.ok(expected);
        assert.deepEqual(actual.get(job.id), {
          status: "complete",
          states: expected.payload.states,
          generated: expected.payload.generated,
        });
      }
      samples.push({
        route,
        iteration,
        warmup: !saved && iteration === 0,
        wallMs,
        workMs: output.workMs,
        path,
      });
      console.log(JSON.stringify(samples[samples.length - 1]));
    }
  }
  const changed = await Promise.all(
    sources.map(
      async (file) => digest(await readFile(file.path)) === file.sha256,
    ),
  );
  assert.ok(changed.every(Boolean), "Sources changed during timing");
  const summaries = routes.map((route) => ({
    route,
    ...summarize(
      samples
        .filter((sample) => sample.route === route && !sample.warmup)
        .map((sample) => sample.wallMs),
    ),
  }));
  await write("result.json", {
    status: "complete",
    sourceVersion,
    samples,
    summaries,
    files: await Promise.all(
      files.map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: saved ? "tlc-throughput-replay" : "tlc-throughput", status: "complete", sourceVersion })}\n`,
  );
  console.log(JSON.stringify(summaries));
} catch (error) {
  await write("error.json", {
    status: "error",
    message: String(error),
    samples,
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "tlc-throughput", status: "error", sourceVersion })}\n`,
  );
  throw error;
}
