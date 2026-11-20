import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, loadavg } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { summarize } from "../checker-benchmark/protocol.js";
import { cases, fault } from "../single-model/cases.js";
import { prepare, runTool, verify } from "../single-model/runner.js";
import { digest } from "../usable/provenance.js";
import { fingerprint } from "./provenance.js";

const directory = process.argv[2];
const saved = process.argv[3];
assert.ok(directory);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const source = await fingerprint();
const engines = await Promise.all(
  [
    {
      name: "before",
      path: ".artifacts/profile-tuning/before/idealis_core.wasm",
    },
    {
      name: "after",
      path: "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
    },
  ].map(async (entry) => ({
    ...entry,
    sha256: digest(await readFile(entry.path)),
  })),
);
const baseline = JSON.parse(
  await readFile("evals/runs/0023-tuning-comparison/manifest.json", "utf8"),
);
assert.equal(
  engines[0]?.sha256,
  baseline.binaries.find(
    (entry: { stage: string; backend: string }) =>
      entry.stage === "after" && entry.backend === "wasm",
  ).sha256,
);
const inputs = cases(saved ? "small" : "large").filter(
  (input) => input.name === "counter-product" || input.name === "linear-ring",
);
const files: string[] = [];
const samples: {
  input: string;
  stage: string;
  iteration: number;
  variant: string;
  warmup: boolean;
  wallMs: number;
  apiMs: number;
  path: string;
  sampledMaxRssBytes: number | null;
}[] = [];
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
  if (saved) {
    const previous = JSON.parse(
      await readFile(join(saved, "result.json"), "utf8"),
    );
    const manifest = JSON.parse(
      await readFile(join(saved, "manifest.json"), "utf8"),
    );
    assert.equal(previous.status, "complete");
    assert.equal(previous.sourceVersion, source.sourceVersion);
    assert.deepEqual(manifest.engines, engines);
    assert.deepEqual(
      JSON.parse(await readFile(join(saved, "inputs.json"), "utf8")),
      cases("large").filter(
        (input) =>
          input.name === "counter-product" || input.name === "linear-ring",
      ),
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
          (sample: (typeof samples)[number]) =>
            `${sample.input}/${sample.stage}/${sample.iteration}/${sample.variant}`,
        )
        .sort(),
      inputs
        .flatMap((input) =>
          engines.flatMap((engine) =>
            [0, 1, 2, 3].map(
              (iteration) => `${input.name}/${engine.name}/${iteration}/normal`,
            ),
          ),
        )
        .sort(),
    );
    for (const sample of previous.samples as typeof samples) {
      const input = cases("large").find((input) => input.name === sample.input);
      const engine = engines.find((entry) => entry.name === sample.stage);
      assert.ok(input && engine);
      const actual: Awaited<ReturnType<typeof runTool>> = JSON.parse(
        await readFile(join(saved, sample.path), "utf8"),
      );
      verify(actual, input, "complete");
      assert.equal(actual.route, "wasm");
      assert.equal(actual.args.at(-1), resolve(engine.path));
      assert.equal(actual.wallMs, sample.wallMs);
      assert.ok(
        typeof actual.output === "object" &&
          actual.output !== null &&
          "apiMs" in actual.output,
      );
      assert.equal(actual.output.apiMs, sample.apiMs);
      assert.equal(actual.sampledMaxRssBytes, sample.sampledMaxRssBytes);
      assert.equal(sample.warmup, sample.iteration === 0);
    }
  }
  await write("manifest.json", {
    plan: "0024-profile-tuning",
    ...source,
    engines,
    saved: saved ?? null,
    startedAt: new Date().toISOString(),
    commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    node: process.version,
    cpu: cpus()[0]?.model,
    loadAverage: loadavg(),
    warmup: saved ? 0 : 1,
    measurements: saved ? 0 : 3,
    timing:
      "Fresh Node, alternating before/after per iteration. Wall: process spawn through exit. API: IR validation/serialization, initial generation and search. No profiler enabled.",
  });
  await write("inputs.json", inputs);
  for (const input of inputs) {
    await mkdir(join(directory, input.name));
    for (let iteration = 0; iteration < (saved ? 1 : 4); iteration++) {
      for (const engine of iteration % 2 ? [...engines].reverse() : engines) {
        for (const variant of saved ? ["normal", "fault"] : ["normal"]) {
          const selected = variant === "fault" ? fault(input) : input;
          const path = `${input.name}/${engine.name}-${iteration}-${variant}`;
          await prepare(join(directory, path), selected);
          const module = /^---- MODULE (\w+) ----/m.exec(selected.source)?.[1];
          for (const file of ["input.json", `${module}.tla`, `${module}.cfg`])
            files.push(`${path}/${file}`);
          console.log(`Starting ${path}`);
          const actual = await runTool(
            "wasm",
            join(directory, path),
            selected,
            "",
            "",
            undefined,
            engine.path,
          );
          await write(`${path}/sample.json`, actual);
          const result = verify(
            actual,
            selected,
            variant === "fault" ? "invariant-violation" : "complete",
          );
          const key = `${input.name}/${variant}`;
          if (expected.has(key))
            assert.deepEqual(
              result,
              expected.get(key),
              "Results and work must be identical",
            );
          else expected.set(key, result);
          assert.ok(
            typeof actual.output === "object" &&
              actual.output !== null &&
              "apiMs" in actual.output &&
              typeof actual.output.apiMs === "number",
          );
          const sample = {
            input: input.name,
            stage: engine.name,
            iteration,
            variant,
            warmup: !saved && iteration === 0,
            wallMs: actual.wallMs,
            apiMs: actual.output.apiMs,
            path: `${path}/sample.json`,
            sampledMaxRssBytes: actual.sampledMaxRssBytes,
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
  }
  files.push("progress.jsonl");
  assert.equal(samples.length, saved ? 8 : 16);
  assert.equal((await fingerprint()).sourceVersion, source.sourceVersion);
  for (const engine of engines)
    assert.equal(digest(await readFile(engine.path)), engine.sha256);
  const summaries = saved
    ? []
    : inputs.flatMap((input) =>
        engines.map((engine) => {
          const selected = samples.filter(
            (sample) =>
              sample.input === input.name &&
              sample.stage === engine.name &&
              !sample.warmup,
          );
          assert.equal(selected.length, 3);
          return {
            input: input.name,
            stage: engine.name,
            wall: summarize(selected.map((sample) => sample.wallMs)),
            api: summarize(selected.map((sample) => sample.apiMs)),
          };
        }),
      );
  await write("result.json", {
    status: "complete",
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
    `${JSON.stringify({ runId: basename(directory), kind: saved ? "profile-scalar-replay" : "profile-scalar", status: "complete", sourceVersion: source.sourceVersion })}\n`,
  );
} catch (error) {
  await write("error.json", {
    status: "error",
    message: String(error),
    samples,
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "profile-scalar", status: "error", sourceVersion: source.sourceVersion })}\n`,
  );
  throw error;
}
