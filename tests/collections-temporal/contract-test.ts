import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { check } from "tla-ts";
import {
  bounds,
  cases,
} from "../../evals/experiments/collections-temporal/cases.js";
import { traceInput } from "../../evals/experiments/collections-temporal/tlc.js";
import { startNative } from "../../evals/experiments/usable/native.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import {
  checkSafety,
  decodeSafetyResult,
} from "../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
for (const input of cases)
  test(input.name, async () => {
    const program = compileTlaSafety(input);
    const native = startNative();
    try {
      const response = await native.request({
        version: 1,
        request: {
          kind: "check-safety",
          program,
          bounds: input.bounds ?? bounds,
        },
      });
      assert.ok(isRecord(response));
      if (input.expected === "error") {
        assert.equal(response.kind, "error", JSON.stringify(response));
        assert.throws(() => checkSafety(core, program, input.bounds ?? bounds));
      } else {
        assert.equal(response.kind, "safety", JSON.stringify(response));
        const actual = decodeSafetyResult(response.result);
        assert.deepEqual(
          checkSafety(core, program, input.bounds ?? bounds),
          actual,
        );
        assert.equal(actual.status, input.expected, JSON.stringify(actual));
        if (actual.violation?.trace.length && actual.status !== "deadlock") {
          const forced = traceInput(input, program, actual);
          const witness = await check({
            ...forced,
            livenessCheck: "final",
          });
          assert.equal(
            witness.status,
            forced.expected === "complete" ? "success" : forced.expected,
            JSON.stringify(witness),
          );
          assert.equal(witness.distinctStates, actual.violation.trace.length);
        }
        if (actual.status === "liveness-violation") {
          assert.ok(
            actual.violation &&
              actual.violation.loopStart !== undefined &&
              actual.violation.loopStart < actual.violation.trace.length,
          );
          assert.equal(actual.violation.trace[0]?.action, null);
          assert.ok(actual.violation.loopAction);
        }
      }
      if (input.bounds || input.expected === "empty") return;
      if (input.expected === "error") {
        assert.equal(
          (await check({ ...input, livenessCheck: "final" })).status,
          "error",
        );
        return;
      }
      const reference = await check({ ...input, livenessCheck: "final" });
      assert.equal(
        reference.status,
        input.expected === "complete"
          ? "success"
          : input.expected === "property-violation"
            ? "liveness-violation"
            : input.expected,
        JSON.stringify(reference),
      );
      if (input.expected === "complete" && response.kind === "safety") {
        const actual = decodeSafetyResult(response.result);
        assert.equal(actual.states, reference.distinctStates);
        assert.equal(actual.generated, reference.generatedStates);
      }
    } finally {
      await native.close();
    }
  });
