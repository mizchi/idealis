import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { checkSafety } from "../../../packages/idealis/src/safety.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import {
  bounds,
  counter,
  joint,
  monotone,
  nodeSets,
  plain,
  recordNodes,
} from "./cases.js";

const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const inputs = [];
for (const [input, counts] of [
  [counter(5, 3), [1024, 56, 5, 141]],
  [counter(7, 5), [279936, 792, 7, 2773]],
  [joint(5), [1024, 56, 10, 281]],
  [recordNodes(5), [7776, 252, 10, 1513]],
  [nodeSets(5), [1024, 56, 10, 281]],
] as const) {
  for (const [mode, selected, states] of [
    ["plain", plain(input), counts[0]],
    ["symmetry", input, counts[1]],
  ] as const) {
    const program = compileTlaSource(core, selected);
    const generated = states * counts[2] + 1;
    const result = checkSafety(core, program, bounds);
    assert.equal(result.status, "complete");
    assert.equal(result.states, states);
    assert.equal(result.generated, generated);
    inputs.push({ name: `${input.name}/${mode}`, states, generated, program });
    if (mode === "symmetry") {
      const options = { symmetryActions: true };
      const reduced = checkSafety(core, program, bounds, options);
      assert.equal(reduced.status, "complete");
      assert.equal(reduced.states, states);
      assert.equal(reduced.generated, counts[3]);
      inputs.push({
        name: `${input.name}/actions`,
        states,
        generated: counts[3],
        program,
        options,
      });
    }
  }
}
const large = recordNodes(12);
const largeProgram = compileTlaSource(core, large);
for (const symmetryActions of [false, true]) {
  const options = { symmetryActions };
  const generated = symmetryActions ? 52417 : 148513;
  const result = checkSafety(core, largeProgram, bounds, options);
  assert.equal(result.status, "complete");
  assert.equal(result.states, 6188);
  assert.equal(result.generated, generated);
  for (const workers of [1, 8])
    inputs.push({
      name: `${large.name}/${symmetryActions ? "actions" : "symmetry"}-${workers}`,
      states: 6188,
      generated,
      program: largeProgram,
      options: { ...options, workers },
    });
}
const monotoneProgram = compileTlaSource(core, monotone(10));
for (const partialOrder of [false, true]) {
  const options = { partialOrder };
  const states = partialOrder ? 14 : 4096;
  const generated = partialOrder ? 15 : 24577;
  const result = checkSafety(core, monotoneProgram, bounds, options);
  assert.equal(result.status, "complete");
  assert.equal(result.states, states);
  assert.equal(result.generated, generated);
  inputs.push({
    name: `monotone-10/${partialOrder ? "por" : "plain"}`,
    states,
    generated,
    program: monotoneProgram,
    options,
  });
}
await mkdir(".artifacts/symmetry", { recursive: true });
await writeFile(
  ".artifacts/symmetry/inputs.json",
  `${JSON.stringify(inputs, null, 2)}\n`,
);
for (const [fixture, selected] of [
  [
    "symmetry-fixtures.json",
    inputs.filter(
      (input) =>
        !input.name.startsWith("record-12-") &&
        !input.name.startsWith("monotone-10/"),
    ),
  ],
  [
    "reduction-fixtures.json",
    inputs.filter(
      (input) =>
        input.name.startsWith("record-12-") ||
        input.name.startsWith("monotone-10/"),
    ),
  ],
] as const) {
  const path = `crates/rtlc/src/${fixture}`;
  if (process.argv.includes("--write-fixture"))
    await writeFile(path, `${JSON.stringify(selected, null, 2)}\n`);
  else assert.deepEqual(JSON.parse(await readFile(path, "utf8")), selected);
}
console.log(`Verified ${inputs.length} prepared benchmark inputs`);
