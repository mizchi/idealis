import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  bounds,
  corpus,
  counter,
  partialOrderCorpus,
  plain,
} from "../../evals/experiments/symmetry/cases.js";
import { startNative } from "../../evals/experiments/usable/native.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import {
  checkSafety,
  decodeSafetyResult,
} from "../../packages/idealis/src/safety.js";
import { compileTlaSource } from "../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);

test("partial order is opt-in and portable with strict fallback and option validation", async () => {
  const native = startNative();
  try {
    for (const input of partialOrderCorpus()) {
      const program = compileTlaSource(core, input);
      const actual = checkSafety(core, program, bounds, { partialOrder: true });
      const original = checkSafety(core, program, bounds);
      assert.equal(actual.status, original.status, input.name);
      assert.ok(actual.states <= original.states);
      if (input.name.includes("fallback")) assert.deepEqual(actual, original);
      for (const workers of [1, 8]) {
        const response = await native.request({
          version: 1,
          request: {
            kind: "check-safety",
            program,
            bounds,
            options: { partialOrder: true, workers },
          },
        });
        assert.ok(isRecord(response) && response.kind === "safety");
        assert.deepEqual(decodeSafetyResult(response.result), actual);
      }
    }
    const program = compileTlaSource(core, counter(3, 2));
    assert.throws(
      () => checkSafety(core, program, bounds, { partialOrder: true }),
      PortableCoreError,
    );
    for (const workers of [0, 257])
      assert.throws(
        () => checkSafety(core, program, bounds, { workers }),
        PortableCoreError,
      );
  } finally {
    await native.close();
  }
});

test("restricted symmetry preserves native/Wasm verdicts, concrete traces and narrow frontier fallback", async () => {
  const native = startNative();
  try {
    for (const input of corpus()) {
      const program = compileTlaSource(core, input);
      const actual = checkSafety(core, program, bounds);
      const original = checkSafety(
        core,
        compileTlaSource(core, plain(input)),
        bounds,
      );
      assert.equal(actual.status, original.status, input.name);
      assert.equal(
        actual.violation?.trace.length,
        original.violation?.trace.length,
        input.name,
      );
      assert.ok(actual.states <= original.states);
      for (const workers of [1, 8]) {
        const response = await native.request({
          version: 1,
          request: { kind: "check-safety-parallel", program, bounds, workers },
        });
        assert.ok(
          isRecord(response) && response.kind === "safety",
          JSON.stringify(response),
        );
        assert.deepEqual(
          decodeSafetyResult(response.result),
          actual,
          input.name,
        );
      }
      if (actual.status === "complete") assert.ok(actual.states > 0);
      const actions = checkSafety(core, program, bounds, {
        symmetryActions: true,
      });
      assert.equal(actions.status, actual.status);
      assert.equal(actions.states, actual.states);
      assert.ok(actions.generated <= actual.generated);
      assert.equal(
        actions.violation?.trace.length,
        actual.violation?.trace.length,
      );
      const response = await native.request({
        version: 1,
        request: {
          kind: "check-safety",
          program,
          bounds,
          options: { symmetryActions: true },
        },
      });
      assert.ok(isRecord(response) && response.kind === "safety");
      assert.deepEqual(decodeSafetyResult(response.result), actions);
    }
  } finally {
    await native.close();
  }
});

test("symmetry rejects unsupported groups, runtime shapes, order-sensitive expressions and combinations", () => {
  const input = counter(3, 2);
  const program = compileTlaSource(core, input);
  for (const modelValues of [
    [],
    ["p0"],
    ["p0", "p0"],
    ["n_a", "n_b"],
    ["bad name", "p0"],
    Array.from({ length: 65 }, (_, index) => `p${index}`),
  ]) {
    assert.throws(
      () =>
        checkSafety(core, { ...program, symmetry: { modelValues } }, bounds),
      PortableCoreError,
    );
  }
  for (const altered of [
    {
      ...input,
      source: input.source.replace("|-> 0]", "|-> [value |-> [nested |-> 0]]]"),
    },
    { ...input, source: input.source.replace("|-> 0]", "|-> node]") },
    { ...input, source: input.source.replace("|-> 0]", '|-> "p0"]') },
    {
      ...input,
      source: input.source.replace(
        "|-> 0]",
        "|-> CHOOSE candidate \\in Nodes: TRUE]",
      ),
    },
  ]) {
    assert.throws(
      () => checkSafety(core, compileTlaSource(core, altered), bounds),
      PortableCoreError,
    );
  }
  assert.throws(
    () =>
      core.request({
        version: 1,
        request: { kind: "check-safety-coverage", program, bounds },
      }),
    PortableCoreError,
  );
  assert.throws(
    () =>
      checkSafety(
        core,
        {
          ...program,
          actionConstraints: [
            {
              name: "C",
              expression: {
                op: "literal",
                value: { type: "bool", value: true },
              },
            },
          ],
        },
        bounds,
      ),
    PortableCoreError,
  );
  assert.throws(
    () =>
      checkSafety(
        core,
        {
          ...program,
          fairness: [
            {
              kind: "atom",
              expression: {
                op: "literal",
                value: { type: "bool", value: true },
              },
            },
          ],
        },
        bounds,
      ),
    PortableCoreError,
  );
});

test("the 64-node boundary uses row reduction without permutation enumeration", () => {
  const result = checkSafety(
    core,
    compileTlaSource(core, counter(64, 1)),
    bounds,
  );
  assert.equal(result.status, "complete");
  assert.equal(result.states, 65);
  assert.equal(result.generated, 4161);
  assert.equal(result.depth, 64);
  const actions = checkSafety(
    core,
    compileTlaSource(core, counter(64, 1)),
    bounds,
    { symmetryActions: true },
  );
  assert.equal(actions.status, "complete");
  assert.equal(actions.states, 65);
  assert.equal(actions.generated, 129);
});
