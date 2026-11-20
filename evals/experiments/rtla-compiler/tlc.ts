import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import { cases } from "../collections-temporal/cases.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { differential } from "../tlc-differential/differential.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";

const directory = process.argv[2];
const java = process.argv[3];
const jar = process.argv[4];
assert.ok(directory && java && jar);
const selected = [
  "set-delivery",
  "set-fault",
  "function-updates",
  "record-updates",
  "except-sequential-at",
  "sequence-queue",
  "filter-map-powerset",
  "tuple-pattern",
  "independent-existentials",
  "let-lexical-scope",
  "initial-property-failure",
  "initial-property-only",
  "action-property",
  "always-state",
  "eventually-stuttering",
  "eventually-WF",
  "leads-to-WF",
  "WF-intermittent",
  "SF-intermittent",
  "deadlock-before-temporal",
];
const inputs = selected.map((name) => {
  const input = cases.find((input) => input.name === name);
  assert.ok(input);
  return {
    name,
    family: "graph" as const,
    source: input.source.replace(
      /---- MODULE \w+ ----/,
      "---- MODULE Differential ----",
    ),
    config: input.config,
  };
});
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const source = await fingerprint();
const manifest = {
  sourceVersion: source.sourceVersion,
  sources: source.sources,
  inputs,
  jarSha256: digest(await readFile(jar)),
  harnessSha256: digest(
    await readFile("evals/experiments/rtla-compiler/tlc.ts"),
  ),
  moduleHeaderRenamed: true,
  frontend: "rtla-compiler",
  performanceMeasurement: false,
};
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(manifest, null, 2),
  { flag: "wx" },
);
const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const native = startNative();
const reports = [];
try {
  for (const input of inputs) {
    await mkdir(join(directory, input.name));
    const report = await differential(
      input,
      core,
      native,
      java,
      jar,
      join(directory, input.name),
      (input) => compileTlaSource(core, input),
    );
    reports.push(report);
    console.log(`${input.name}: ${report.differences.join(",") || "matched"}`);
  }
  await writeFile(
    join(directory, "reports.json"),
    JSON.stringify(reports, null, 2),
    { flag: "wx" },
  );
  assert.ok(reports.every((report) => report.differences.length === 0));
  for (const file of source.sources)
    assert.equal(digest(await readFile(file.path)), file.sha256);
  const result = {
    status: "complete",
    sourceVersion: source.sourceVersion,
    cases: reports.length,
    matched: reports.length,
    frontend: "rtla-compiler",
    files: await Promise.all(
      ["manifest.json", "reports.json"].map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  };
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(result, null, 2),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "rtla-compiler-tlc", status: "complete", sourceVersion: source.sourceVersion })}\n`,
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", message: String(error), reports },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "rtla-compiler-tlc", status: "error", sourceVersion: source.sourceVersion })}\n`,
  );
  throw error;
} finally {
  await native.close();
}
