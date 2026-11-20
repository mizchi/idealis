import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { generatedCases } from "../../evals/experiments/recursive-actions/generated.js";
import { startNative } from "../../evals/experiments/usable/native.js";
import type { TlaSourceInput } from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyProgram } from "../../packages/idealis/src/safety-program.js";
import { checkSafety } from "../../packages/idealis/src/safety.js";
import { compileTlaSource } from "../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

interface Case extends TlaSourceInput {
  readonly irVersion: 2;
  readonly name: string;
  readonly status: string;
  readonly states: number;
  readonly generated: number;
}
const fixed: readonly Case[] = JSON.parse(
  await readFile("crates/rtla-compiler/tests/recursive-actions.json", "utf8"),
);
interface PropertyPlan {
  readonly developmentSeed: number;
  readonly confirmationSeed: number;
  readonly casesPerSeed: number;
}
const plan: PropertyPlan = JSON.parse(
  await readFile("evals/plans/0071-recursive-action-properties.json", "utf8"),
);
const inputs = [
  ...fixed,
  ...generatedCases(plan.developmentSeed, plan.casesPerSeed),
  ...generatedCases(plan.confirmationSeed, plan.casesPerSeed),
];
const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const bounds = {
  maxDepth: 1000,
  maxStates: 2000,
  maxWork: 1_000_000,
  maxTemporalNodes: 10000,
};
for (const input of inputs)
  test(`recursive actions native/Wasm contract: ${input.name}`, async () => {
    const source = {
      source: input.source,
      config: input.config,
      irVersion: input.irVersion,
      ...(input.modules ? { modules: input.modules } : {}),
    };
    const native = startNative();
    try {
      if (input.status === "error") {
        assert.throws(() => compileTlaSource(core, input), PortableCoreError);
        const result = await native.request({
          version: 1,
          request: { kind: "compile-tla", ...source },
        });
        assert.ok(isRecord(result) && result.kind === "error");
        return;
      }
      const program = compileTlaSource(core, input);
      assert.equal(program.version, 2);
      assert.deepEqual(decodeSafetyProgram(program), program);
      assert.deepEqual(
        await native.request({
          version: 1,
          request: { kind: "compile-tla", ...source },
        }),
        { kind: "compiled", program },
      );
      const result = checkSafety(core, program, bounds);
      assert.deepEqual(
        await native.request({
          version: 1,
          request: { kind: "check-safety", program, bounds },
        }),
        { kind: "safety", result },
      );
      assert.equal(result.status, input.status);
      if (input.status === "bounded") {
        assert.equal(result.reason, "evaluation");
        assert.equal(result.violation, null);
      } else {
        assert.equal(result.states, input.states);
        assert.equal(result.generated, input.generated);
      }
      const partial = checkSafety(core, program, { ...bounds, maxWork: 1 });
      assert.equal(partial.status, "bounded");
      assert.equal(partial.reason, "work");
      assert.equal(partial.work, 1);
    } finally {
      await native.close();
    }
  });

test("action definitions and calls have strict wire discriminators", () => {
  const input = inputs.find((input) => input.name === "tail-4");
  assert.ok(input);
  const program = compileTlaSource(core, input);
  assert.ok(program.version === 2);
  const index = program.definitions.findIndex(
    (definition) => definition.bodyKind === "action",
  );
  assert.ok(index >= 0);
  const definition = program.definitions[index];
  assert.ok(definition);
  for (const invalid of [
    { ...definition, bodyKind: "expression" },
    {
      ...definition,
      bodyKind: ["expression"],
      body: { op: "literal", value: { type: "bool", value: true } },
    },
    {
      ...definition,
      body: { op: "literal", value: { type: "bool", value: true } },
    },
    { ...definition, recursive: "true" },
    { ...definition, unexpected: true },
  ])
    assert.throws(
      () =>
        decodeSafetyProgram({
          ...program,
          definitions: program.definitions.map((definition, position) =>
            position === index ? invalid : definition,
          ),
        }),
      PortableCoreError,
    );
  assert.throws(
    () => decodeSafetyProgram({ ...program, version: 1 }),
    PortableCoreError,
  );
});
