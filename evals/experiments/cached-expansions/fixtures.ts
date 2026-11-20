import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { checkSafety } from "../../../packages/idealis/src/safety.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import { bounds, config, names } from "../quint-generated/cases.js";

const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const inputs = await Promise.all(
  names.map(async (name) => ({
    name: String(name),
    source: await readFile(
      `fixtures/quint-generated/generated/${name}.tla`,
      "utf8",
    ),
    config: config("safety"),
    provenance: "unchanged Quint 0.33.1 output",
  })),
);
const fold = await readFile(
  "fixtures/quint-generated/generated/SetFold.tla",
  "utf8",
);
for (const size of [16, 256, 4096]) {
  const source = fold
    .replace(
      "step == total' := total",
      `Heavy == LET Add(acc, element) == acc + element IN ApaFoldSet(Add, 0, 1..${size})\n\nstep == total' := Heavy`,
    )
    .replace("safety == total = 6", "safety == total >= 0");
  inputs.push({
    name: `derived-fold-${size}`,
    source,
    config: config("safety"),
    provenance: `derived from SetFold: Next recomputes ApaFoldSet over 1..${size}; current invariant total>=0`,
  });
}
const fixtures = inputs.map((input) => {
  const program = compileTlaSource(core, {
    source: input.source,
    config: input.config,
    irVersion: 2,
  });
  const result = checkSafety(core, program, bounds);
  assert.equal(result.status, "complete", input.name);
  return {
    ...input,
    program,
    states: result.states,
    generated: result.generated,
  };
});
await writeFile(
  "crates/rtlc/tests/expansions-fixtures.json",
  `${JSON.stringify(fixtures, null, 2)}\n`,
);
console.log(
  fixtures.map((input) => ({
    name: input.name,
    states: input.states,
    generated: input.generated,
  })),
);
