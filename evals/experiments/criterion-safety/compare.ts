import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, loadavg, totalmem } from "node:os";
import { basename, dirname, join } from "node:path";
import { order, summarize } from "../checker-benchmark/protocol.js";
import { cases, fault } from "../single-model/cases.js";
import { prepare, runTool, verify } from "../single-model/runner.js";
import { digest } from "../usable/provenance.js";
import { binaries, fingerprint, routes } from "./protocol.js";

interface Sample {
  readonly input: string;
  readonly route: string;
  readonly iteration: number;
  readonly warmup: boolean;
  readonly path: string;
  readonly wallMs: number;
  readonly apiMs: number;
  readonly frontendMs: number;
  readonly engineStartupMs: number;
  readonly work: number;
  readonly sampledMaxRssBytes: number | null;
}
const directory = process.argv[2];
const saved = process.argv[3];
assert.ok(directory);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const source = await fingerprint();
const engines = await binaries();
const samples: Sample[] = [];
const files: string[] = [];
const expected = new Map<string, unknown>();
const write = async (path: string, value: unknown) => {
  await writeFile(
    join(directory, path),
    `${JSON.stringify(value, null, 2)}\n`,
    { flag: "wx" },
  );
  files.push(path);
};
try {
  if (saved && saved !== "controls") {
    const previous = JSON.parse(
      await readFile(join(saved, "result.json"), "utf8"),
    );
    const manifest = JSON.parse(
      await readFile(join(saved, "manifest.json"), "utf8"),
    );
    assert.equal(previous.status, "complete");
    assert.equal(previous.sourceVersion, source.sourceVersion);
    assert.deepEqual(manifest.binaries, engines);
    assert.equal(manifest.plan, "0023-criterion-safety");
    assert.deepEqual(
      JSON.parse(await readFile(join(saved, "cases.json"), "utf8")),
      cases("large"),
    );
    for (const file of previous.files) {
      assert.ok(
        typeof file.path === "string" &&
          !file.path.includes("..") &&
          !file.path.startsWith("/"),
      );
      assert.equal(
        digest(await readFile(join(saved, file.path))),
        file.sha256,
        file.path,
      );
    }
    assert.deepEqual(
      previous.samples
        .map(
          (sample: Sample) =>
            `${sample.input}/${sample.route}/${sample.iteration}`,
        )
        .sort(),
      cases("large")
        .flatMap((input) =>
          routes.flatMap((route) =>
            [0, 1, 2, 3].map(
              (iteration) => `${input.name}/${route.name}/${iteration}`,
            ),
          ),
        )
        .sort(),
    );
    for (const sample of previous.samples as Sample[]) {
      const input = cases("large").find((input) => input.name === sample.input);
      const route = routes.find((route) => route.name === sample.route);
      assert.ok(input && route);
      const engine = engines.find(
        (entry) =>
          entry.stage === route.stage && entry.backend === route.backend,
      );
      assert.ok(engine);
      const actual: Awaited<ReturnType<typeof runTool>> = JSON.parse(
        await readFile(join(saved, sample.path), "utf8"),
      );
      verify(actual, input, "complete");
      assert.equal(actual.route, route.backend);
      assert.equal(actual.args.at(-1), `${process.cwd()}/${engine.path}`);
      assert.equal(
        actual.args[4],
        route.workers === undefined ? "" : String(route.workers),
      );
      assert.equal(sample.wallMs, actual.wallMs);
      assert.equal(sample.sampledMaxRssBytes, actual.sampledMaxRssBytes);
      assert.deepEqual(
        {
          apiMs: sample.apiMs,
          frontendMs: sample.frontendMs,
          engineStartupMs: sample.engineStartupMs,
          work: sample.work,
        },
        (() => {
          const output = actual.output as {
            apiMs: number;
            frontendMs: number;
            engineStartupMs: number;
            result: { work: number };
          };
          return {
            apiMs: output.apiMs,
            frontendMs: output.frontendMs,
            engineStartupMs: output.engineStartupMs,
            work: output.result.work,
          };
        })(),
      );
      assert.equal(sample.warmup, sample.iteration === 0);
    }
  }
  await write("manifest.json", {
    plan: "0023-criterion-safety",
    ...source,
    binaries: engines,
    beforeEvidence: "evals/runs/0022-readme-comparison/manifest.json",
    commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    dirtyPaths: execFileSync("git", ["status", "--short"]).toString().trim(),
    startedAt: new Date().toISOString(),
    node: process.version,
    cpu: cpus()[0]?.model,
    memory: totalmem(),
    loadAverage: loadavg(),
    rust: execFileSync("rustc", ["--version"]).toString().trim(),
    saved: saved ?? null,
    measurements: saved ? 0 : 3,
    warmup: saved ? 0 : 1,
    rssIntervalMs: 200,
    timing:
      "Fresh Node per sample. Wall: spawn through exit; API: IR validation, serialization, initial generation and search. Frontend parse/compile and engine startup recorded separately. Wasm runs in Node.",
  });
  const inputs = cases(saved ? "small" : "large");
  await write("cases.json", inputs);
  for (const input of inputs) {
    await mkdir(join(directory, input.name));
    for (let iteration = 0; iteration < (saved ? 1 : 4); iteration++) {
      for (const route of order(routes, iteration)) {
        const engine = engines.find(
          (entry) =>
            entry.stage === route.stage && entry.backend === route.backend,
        );
        assert.ok(engine);
        for (const variant of saved ? ["normal", "fault"] : ["normal"]) {
          const selected = variant === "fault" ? fault(input) : input;
          const path = `${input.name}/${route.name}-${iteration}-${variant}`;
          await prepare(join(directory, path), selected);
          const moduleName = /^---- MODULE (\w+) ----/m.exec(
            selected.source,
          )?.[1];
          for (const file of [
            "input.json",
            `${moduleName}.tla`,
            `${moduleName}.cfg`,
          ])
            files.push(`${path}/${file}`);
          console.log(`Starting ${path}`);
          const actual = await runTool(
            route.backend,
            join(directory, path),
            selected,
            "",
            "",
            route.workers,
            engine.path,
          );
          await write(`${path}/sample.json`, actual);
          const result = verify(
            actual,
            selected,
            variant === "fault" ? "invariant-violation" : "complete",
          );
          assert.ok(
            result && "work" in result && typeof result.work === "number",
          );
          const key = `${input.name}/${variant}`;
          const comparable = { ...result, work: 0 };
          if (expected.has(key))
            assert.deepEqual(comparable, expected.get(key), key);
          else expected.set(key, comparable);
          const output = actual.output as {
            apiMs: number;
            frontendMs: number;
            engineStartupMs: number;
          };
          const sample: Sample = {
            input: input.name,
            route: route.name,
            iteration,
            warmup: !saved && iteration === 0,
            path: `${path}/sample.json`,
            wallMs: actual.wallMs,
            apiMs: output.apiMs,
            frontendMs: output.frontendMs,
            engineStartupMs: output.engineStartupMs,
            work: result.work,
            sampledMaxRssBytes: actual.sampledMaxRssBytes,
          };
          samples.push(sample);
          await appendFile(
            join(directory, "progress.jsonl"),
            `${JSON.stringify({ ...sample, variant })}\n`,
          );
          console.log(JSON.stringify(sample));
        }
      }
    }
  }
  files.push("progress.jsonl");
  assert.equal(
    (await fingerprint()).sourceVersion,
    source.sourceVersion,
    "Sources changed during measurement",
  );
  assert.deepEqual(
    await binaries(),
    engines,
    "Binaries changed during measurement",
  );
  assert.equal(samples.length, saved ? 48 : 96);
  const summaries = saved
    ? []
    : inputs.flatMap((input) =>
        routes.map((route) => {
          const selected = samples.filter(
            (sample) =>
              sample.input === input.name &&
              sample.route === route.name &&
              !sample.warmup,
          );
          assert.equal(selected.length, 3);
          const rss = selected.flatMap((sample) =>
            sample.sampledMaxRssBytes === null
              ? []
              : [sample.sampledMaxRssBytes],
          );
          return {
            input: input.name,
            route: route.name,
            wall: summarize(selected.map((sample) => sample.wallMs)),
            api: summarize(selected.map((sample) => sample.apiMs)),
            work: selected.map((sample) => sample.work),
            sampledMaxRssBytes: rss.length ? Math.max(...rss) : null,
          };
        }),
      );
  await write("result.json", {
    status: "complete",
    sourceVersion: source.sourceVersion,
    loadAverage: loadavg(),
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
    `${JSON.stringify({ runId: basename(directory), kind: saved === "controls" ? "criterion-controls" : saved ? "criterion-compare-replay" : "criterion-compare", status: "complete", sourceVersion: source.sourceVersion })}\n`,
  );
} catch (error) {
  await write("error.json", {
    status: "error",
    message: String(error),
    samples,
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "criterion-compare", status: "error", sourceVersion: source.sourceVersion })}\n`,
  );
  throw error;
}
