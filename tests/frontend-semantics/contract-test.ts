import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import type { FrontendCase } from "../../evals/experiments/frontend-semantics/cases.js";
import { startNative } from "../../evals/experiments/usable/native.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyProgram } from "../../packages/idealis/src/safety-program.js";
import { checkSafety } from "../../packages/idealis/src/safety.js";
import { compileTlaSource } from "../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

const inputs: readonly FrontendCase[] = JSON.parse(
  await readFile("crates/rtla-compiler/tests/frontend-semantics.json", "utf8"),
);
const wasm = await createPortableWasm(
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
  test(`frontend semantics native/Wasm: ${input.name}`, async () => {
    const native = startNative();
    try {
      const source = {
        source: input.source,
        config: input.config,
        irVersion: input.irVersion,
      };
      if (input.expected === "input-error") {
        assert.throws(() => compileTlaSource(wasm, source), PortableCoreError);
        const result = await native.request({
          version: 1,
          request: { kind: "compile-tla", ...source },
        });
        assert.ok(isRecord(result) && result.kind === "error");
        return;
      }
      const program = compileTlaSource(wasm, source);
      assert.deepEqual(decodeSafetyProgram(program), program);
      assert.deepEqual(
        await native.request({
          version: 1,
          request: { kind: "compile-tla", ...source },
        }),
        { kind: "compiled", program },
      );
      if (input.expected === "evaluation-error") {
        assert.throws(
          () => checkSafety(wasm, program, bounds),
          PortableCoreError,
        );
        const result = await native.request({
          version: 1,
          request: { kind: "check-safety", program, bounds },
        });
        assert.ok(isRecord(result) && result.kind === "error");
        return;
      }
      const result = checkSafety(wasm, program, bounds);
      assert.deepEqual(
        await native.request({
          version: 1,
          request: { kind: "check-safety", program, bounds },
        }),
        { kind: "safety", result },
      );
      assert.equal(
        result.status,
        input.expected === "temporal-violation"
          ? "liveness-violation"
          : input.expected,
      );
      if (input.expected === "bounded") {
        assert.equal(result.reason, "evaluation");
        assert.equal(result.violation, null);
      }
      const partial = checkSafety(wasm, program, { ...bounds, maxWork: 1 });
      assert.equal(partial.status, "bounded");
      assert.equal(partial.reason, "work");
      assert.equal(partial.work, 1);
    } finally {
      await native.close();
    }
  });
