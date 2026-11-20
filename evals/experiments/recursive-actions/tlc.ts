import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import type { TlaSourceInput } from "../../../packages/idealis/src/contracts.js";
import { runTlc } from "../collections-temporal/tlc.js";
import { digest } from "../usable/provenance.js";

interface Case extends TlaSourceInput {
  readonly name: string;
  readonly status: string;
  readonly states: number;
  readonly generated: number;
}
const [directory, java, jar] = process.argv.slice(2);
assert.ok(directory && java && jar);
await mkdir(directory);
const path = "crates/rtla-compiler/tests/recursive-actions.json";
const bytes = await readFile(path);
const inputs: readonly Case[] = JSON.parse(bytes.toString());
const save = (name: string, value: unknown) =>
  writeFile(`${directory}/${name}`, `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  plan: "0071-recursive-actions",
  path,
  inputSha256: digest(bytes),
  inputs,
  jarSha256: digest(await readFile(jar)),
  driverSha256: digest(
    await readFile("evals/experiments/recursive-actions/tlc.ts"),
  ),
});
const results = [];
for (const [index, input] of inputs.entries()) {
  if (input.status === "bounded") continue;
  const result = await runTlc(
    java,
    jar,
    `${directory}/${index}`,
    input,
    "Differential",
  );
  const differences = [
    ...(result.status === input.status ? [] : ["status"]),
    ...(result.states === null || result.states === input.states
      ? []
      : ["states"]),
    ...(result.generated === null || result.generated === input.generated
      ? []
      : ["generated"]),
  ];
  results.push({ name: input.name, result, differences });
  console.log(
    input.name,
    result.status,
    result.states,
    result.generated,
    differences,
  );
}
await save("results.json", results);
assert.equal(digest(await readFile(path)), digest(bytes));
const differences = results.filter((result) => result.differences.length);
await save("summary.json", { cases: results.length, differences });
assert.equal(differences.length, 0);
