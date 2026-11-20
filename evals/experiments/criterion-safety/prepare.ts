import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { compileTlaSafety } from "../../../packages/idealis/src/tla-safety.js";
import { cases } from "../single-model/cases.js";

const sizes = [12, 12, 4, 10000];
const counts = [
  [20736, 82945],
  [4096, 49153],
  [4096, 24577],
  [10000, 10001],
];
const inputs = cases("large").map((input, index) => {
  const size = sizes[index];
  const count = counts[index];
  assert.ok(size && count);
  const selected = {
    ...input,
    config: input.config.replace(/CONSTANT ([MN])=\d+/, `CONSTANT $1=${size}`),
  };
  return {
    name: input.name,
    source: selected.source,
    config: selected.config,
    states: count[0],
    generated: count[1],
    program: compileTlaSafety(selected),
  };
});
await mkdir(".artifacts/criterion-safety", { recursive: true });
await writeFile(
  ".artifacts/criterion-safety/inputs.json",
  `${JSON.stringify(inputs, null, 2)}\n`,
);

assert.deepEqual(
  JSON.parse(await readFile("crates/rtlc/src/bench-fixtures.json", "utf8")),
  inputs,
  "Regenerate the checked-in benchmark fixture when changing inputs",
);
