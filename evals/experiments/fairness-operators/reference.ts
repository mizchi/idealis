import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type { TlaSourceInput } from "../../../packages/idealis/src/contracts.js";
import { runTlc } from "../collections-temporal/tlc.js";
import { classifyTlc } from "../tlc-differential/compare.js";
import { digest } from "../usable/provenance.js";

interface Input extends TlaSourceInput {
  name: string;
  tlc: string;
}
const directory = process.argv[2];
const java = process.argv[3];
const jar = process.argv[4];
assert.ok(directory && java && jar);
await mkdir(directory, { recursive: false });
const paths = [
  "evals/plans/0048-fairness-operators.md",
  "evals/experiments/fairness-operators/reference.ts",
  "evals/experiments/collections-temporal/tlc.ts",
  "evals/experiments/tlc-differential/compare.ts",
  "crates/rtla-compiler/tests/operators-support.json",
  java,
  jar,
];
const sources = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const inputs: Input[] = JSON.parse(await readFile(paths[4] ?? "", "utf8"));
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "0048-fairness-operators",
      sources,
      inputs,
      kind: "reference-only",
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const reports = [];
for (const input of inputs) {
  const result = await runTlc(
    java,
    jar,
    join(directory, input.name),
    input,
    "Differential",
  );
  const actual = classifyTlc(result);
  const report = {
    name: input.name,
    expected: input.tlc,
    actual,
    states: result.states,
    generated: result.generated,
    matched: actual === input.tlc,
  };
  reports.push(report);
  console.log(
    `${input.name}: ${actual}${report.matched ? "" : ` (expected ${input.tlc})`}`,
  );
}
for (const source of sources)
  assert.equal(digest(await readFile(source.path)), source.sha256);
await writeFile(
  join(directory, "result.json"),
  JSON.stringify(
    {
      status: reports.every((report) => report.matched)
        ? "matched"
        : "differences",
      reports,
    },
    null,
    2,
  ),
  { flag: "wx" },
);
