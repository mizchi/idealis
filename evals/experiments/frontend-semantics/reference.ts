import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { runTlc } from "../collections-temporal/tlc.js";
import { classifyTlc } from "../tlc-differential/compare.js";
import { digest } from "../usable/provenance.js";
import { levelCases } from "./cases.js";
import { temporalCases } from "./lazy-cases.js";

const inputs = [...levelCases, ...temporalCases];
const [directory, java, jar] = process.argv.slice(2);
assert.ok(directory && java && jar);
await mkdir(directory);
const save = (name: string, value: unknown) =>
  writeFile(`${directory}/${name}`, `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const sources = await Promise.all(
  [
    "evals/experiments/frontend-semantics/cases.ts",
    "evals/experiments/frontend-semantics/reference.ts",
    jar,
  ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
await save("manifest.json", {
  plan: "0072-frontend-semantics",
  sources,
  inputs,
});
const results = [];
for (const [index, input] of inputs.entries()) {
  const reference = await runTlc(
    java,
    jar,
    `${directory}/${index}`,
    input,
    "Differential",
  );
  const observed = classifyTlc(reference);
  const matched =
    input.expected === "temporal-violation"
      ? observed === "liveness-violation"
      : observed === input.expected;
  results.push({
    name: input.name,
    expected: input.expected,
    observed,
    matched,
    reference,
  });
  console.log(input.name, observed, matched);
}
await save("result.json", {
  status: results.every((result) => result.matched) ? "complete" : "failed",
  results,
});
for (const source of sources)
  assert.equal(digest(await readFile(source.path)), source.sha256);
assert.ok(results.every((result) => result.matched));
