import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { catalog } from "./catalog.js";
import { costWorkerFiles } from "./worker-files.js";

const output = process.argv[2];
const parent = process.argv[3];
assert.ok(output);
const runner = resolve(
  "evals/experiments/workspace-oracle/rust/target/release/workspace-oracle",
);
const worker = resolve(
  "dist/evals/experiments/workspace-oracle/cost-worker.js",
);
const digest = (bytes: Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");
const save = async (name: string, value: unknown) =>
  writeFile(resolve(output, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
await mkdir(dirname(output), { recursive: true });
await mkdir(output);
if (parent) {
  const manifest = JSON.parse(
    await readFile(resolve(parent, "manifest.json"), "utf8"),
  );
  for (const entry of manifest.fingerprints)
    assert.equal(digest(await readFile(entry.path)), entry.sha256, entry.path);
  const result = JSON.parse(
    await readFile(resolve(parent, "result.json"), "utf8"),
  );
  assert.equal(result.status, "complete");
  for (const file of result.files)
    assert.equal(
      digest(await readFile(resolve(parent, file.path))),
      file.sha256,
      file.path,
    );
  execFileSync(
    runner,
    [
      resolve(parent, "input.json"),
      resolve(output, "replayed"),
      resolve(parent, "measured/check.json"),
    ],
    { stdio: "inherit" },
  );
  await save("result.json", { status: "complete", parent });
} else {
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
  const input = {
    version: 1,
    name: "workspace-equality-cost",
    mode: "reused",
    baseline: { path: process.execPath, args: [worker, "legacy"] },
    candidate: { path: process.execPath, args: [worker, "oracle"] },
    cases,
    config: {
      baseline: "baseline",
      scope:
        "Node reused process roundtrip: legacy strict equality vs snapshot/Wasm oracle/JSON report; includes IPC and serialization",
      samples: 20,
      warmup: 75,
      iterations: 25,
      max_evaluations: 10000,
    },
  };
  await save("input.json", input);
  const fingerprints = await Promise.all(
    [
      runner,
      process.execPath,
      "evals/experiments/workspace-oracle/cost.ts",
      ...costWorkerFiles,
    ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  );
  await save("manifest.json", { plan: "0052", fingerprints });
  execFileSync(
    runner,
    [resolve(output, "input.json"), resolve(output, "measured")],
    { stdio: "inherit" },
  );
  await save("result.json", {
    status: "complete",
    sample: "measured/bench.json",
    files: await Promise.all(
      [
        "input.json",
        "manifest.json",
        "measured/check.json",
        "measured/bench.json",
      ].map(async (path) => ({
        path,
        sha256: digest(await readFile(resolve(output, path))),
      })),
    ),
  });
}
