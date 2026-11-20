import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { order, summarize } from "../checker-benchmark/protocol.js";
import { environment } from "../checker-benchmark/provenance.js";
import { cases } from "../single-model/cases.js";
import { prepare, runTool, verify } from "../single-model/runner.js";
import { digest } from "../usable/provenance.js";
import { sourceFingerprint } from "./provenance.js";

const routes = [
  "native",
  "parallel-1",
  "parallel-2",
  "parallel-4",
  "parallel-8",
] as const;
type Route = (typeof routes)[number];

export interface Sample {
  readonly input: string;
  readonly route: Route;
  readonly iteration: number;
  readonly warmup: boolean;
  readonly status: "complete" | "error";
  readonly wallMs: number;
  readonly work: number | null;
  readonly intervalMs: number | null;
  readonly sampledMaxRssBytes: number | null;
  readonly path: string;
  readonly error: string | null;
}
const java = process.argv[2];
const jar = process.argv[3];
const archive = process.argv[4];
const directory = process.argv[5];
const saved = process.argv[6];
const plan = process.env.IDEALIS_FRONTIER_PLAN ?? "0020-frontier-safety";
assert.ok(plan === "0020-frontier-safety" || plan === "0021-worker-budget");
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
const source = await sourceFingerprint();
const samples: Sample[] = [];
try {
  const tools = await environment(java, jar, archive);
  if (saved) {
    const result = JSON.parse(
      await readFile(join(saved, "result.json"), "utf8"),
    );
    assert.equal(
      result.sourceVersion,
      source.sourceVersion,
      "Check out recorded sources",
    );
    assert.equal(result.status, "complete");
    for (const file of result.files) {
      assert.ok(
        typeof file.path === "string" &&
          !file.path.includes("..") &&
          !file.path.startsWith("/"),
      );
      assert.equal(
        digest(await readFile(join(saved, file.path))),
        file.sha256,
        `Changed evidence: ${file.path}`,
      );
    }
    assert.equal(result.samples.length, 80);
    assert.equal(
      new Set(
        result.samples.map(
          (sample: Sample) =>
            `${sample.input}/${sample.route}/${sample.iteration}`,
        ),
      ).size,
      80,
    );
    assert.deepEqual(
      result.samples
        .map(
          (sample: Sample) =>
            `${sample.input}/${sample.route}/${sample.iteration}`,
        )
        .sort(),
      cases("large")
        .flatMap((input) =>
          routes.flatMap((route) =>
            [0, 1, 2, 3].map(
              (iteration) => `${input.name}/${route}/${iteration}`,
            ),
          ),
        )
        .sort(),
    );
    const manifest = JSON.parse(
      await readFile(join(saved, "manifest.json"), "utf8"),
    );
    assert.equal(manifest.plan, plan);
    assert.deepEqual(manifest.environment.binaries, tools.binaries);
    assert.deepEqual(
      JSON.parse(await readFile(join(saved, "cases.json"), "utf8")),
      cases("large"),
    );
    for (const sample of result.samples) {
      const input = cases("large").find((input) => input.name === sample.input);
      assert.ok(input && sample.status === "complete");
      const actual: Awaited<ReturnType<typeof runTool>> = JSON.parse(
        await readFile(join(saved, sample.path), "utf8"),
      );
      verify(actual, input, "complete");
      assert.equal(actual.wallMs, sample.wallMs);
      const resultOutput = actual.output;
      assert.ok(
        typeof resultOutput === "object" &&
          resultOutput !== null &&
          "result" in resultOutput &&
          typeof resultOutput.result === "object" &&
          resultOutput.result !== null &&
          "work" in resultOutput.result,
      );
      assert.equal(resultOutput.result.work, sample.work);
      assert.equal(actual.route, "native");
      const expectedWorker =
        sample.route === "native" ? undefined : Number(sample.route.slice(9));
      assert.equal(actual.args.length, expectedWorker === undefined ? 4 : 5);
      if (expectedWorker !== undefined)
        assert.equal(actual.args[4], String(expectedWorker));
      assert.equal(actual.sampledMaxRssBytes, sample.sampledMaxRssBytes);
      assert.equal(sample.warmup, sample.iteration === 0);
    }
  }
  const inputs = cases(saved ? "small" : "large");
  await write("manifest.json", {
    plan,
    ...source,
    commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    dirtyPaths: execFileSync("git", ["status", "--short"]).toString().trim(),
    startedAt: new Date().toISOString(),
    environment: tools,
    saved: saved ?? null,
    warmup: saved ? 0 : 1,
    measurements: saved ? 1 : 3,
    rssIntervalMs: 200,
    timing: {
      wall: "Process spawn through exit and JSON decode",
      tlcInterval:
        "stdout receipts: init generation start through success; includes final FP check",
      idealisInterval:
        "checkSafety API, includes IR validation, serialization and initial generation",
    },
    upstreamChannelSha256: digest(
      await readFile("fixtures/channel/Channel.tla"),
    ),
  });
  await write("cases.json", inputs);
  for (const input of inputs) {
    await mkdir(join(directory, input.name));
    for (let iteration = 0; iteration < (saved ? 1 : 4); iteration++) {
      for (const route of order(routes, iteration)) {
        const path = `${input.name}/${route}-${iteration}`;
        await prepare(join(directory, path), input);
        const moduleName = /^---- MODULE (\w+) ----/m.exec(input.source)?.[1];
        for (const file of [
          "input.json",
          `${moduleName}.tla`,
          `${moduleName}.cfg`,
        ])
          files.push(`${path}/${file}`);
        console.log(`Starting ${input.name} ${route} iteration=${iteration}`);
        const actual = await runTool(
          "native",
          join(directory, path),
          input,
          java,
          jar,
          route === "native" ? undefined : Number(route.slice(9)),
        );
        const output = actual.output;
        const intervalMs =
          actual.phases?.initAndSearchMs ??
          (typeof output === "object" &&
          output !== null &&
          "apiMs" in output &&
          typeof output.apiMs === "number"
            ? output.apiMs
            : null);
        await write(`${path}/sample.json`, actual);
        let error: string | null = null;
        try {
          verify(actual, input, "complete");
        } catch (reason) {
          error = String(reason);
        }
        const sample: Sample = {
          input: input.name,
          route,
          iteration,
          warmup: !saved && iteration === 0,
          status: error ? "error" : "complete",
          wallMs: actual.wallMs,
          work:
            typeof output === "object" &&
            output !== null &&
            "result" in output &&
            typeof output.result === "object" &&
            output.result !== null &&
            "work" in output.result &&
            typeof output.result.work === "number"
              ? output.result.work
              : null,
          intervalMs,
          sampledMaxRssBytes: actual.sampledMaxRssBytes,
          path: `${path}/sample.json`,
          error,
        };
        samples.push(sample);
        await appendFile(
          join(directory, "progress.jsonl"),
          `${JSON.stringify(sample)}\n`,
        );
        console.log(JSON.stringify(sample));
      }
    }
  }
  files.push("progress.jsonl");
  assert.equal(
    (await sourceFingerprint()).sourceVersion,
    source.sourceVersion,
    "Sources changed during measurement",
  );
  const summaries = inputs.flatMap((input) =>
    routes.map((route) => {
      const selected = samples.filter(
        (sample) =>
          sample.input === input.name &&
          sample.route === route &&
          !sample.warmup,
      );
      const complete = selected.every((sample) => sample.status === "complete");
      const rss = selected.flatMap((sample) =>
        sample.sampledMaxRssBytes === null ? [] : [sample.sampledMaxRssBytes],
      );
      return {
        input: input.name,
        route,
        status: complete ? "complete" : "error",
        wall: complete
          ? summarize(selected.map((sample) => sample.wallMs))
          : null,
        interval: complete
          ? summarize(
              selected.flatMap((sample) =>
                sample.intervalMs === null ? [] : [sample.intervalMs],
              ),
            )
          : null,
        sampledMaxRssBytes: rss.length > 0 ? Math.max(...rss) : null,
      };
    }),
  );
  const status = samples.every((sample) => sample.status === "complete")
    ? "complete"
    : "incomplete";
  await write("result.json", {
    status,
    sourceVersion: source.sourceVersion,
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
    `${JSON.stringify({ runId: basename(directory), kind: saved ? "frontier-safety-replay" : "frontier-safety", status, sourceVersion: source.sourceVersion })}\n`,
  );
  console.log(JSON.stringify(summaries));
  if (status !== "complete") process.exitCode = 1;
} catch (error) {
  await write("error.json", {
    status: "error",
    message: String(error),
    samples,
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "frontier-safety", status: "error", sourceVersion: source.sourceVersion })}\n`,
  );
  throw error;
}
