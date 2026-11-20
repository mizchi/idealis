import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { test } from "node:test";
import {
  bounds,
  cases as collectionCases,
} from "../../evals/experiments/collections-temporal/cases.js";
import { cases as scalarCases } from "../../evals/experiments/rust-safety/cases.js";
import { cases, fault } from "../../evals/experiments/single-model/cases.js";
import {
  boundaries,
  graphs,
  seeds,
} from "../../evals/experiments/tlc-differential/corpus.js";
import { startNative } from "../../evals/experiments/usable/native.js";
import type {
  SafetyBounds,
  SafetyResult,
} from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyResult } from "../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

const additional = [
  {
    name: "violation-before-other-parent-error",
    source: String.raw`---- MODULE Ordered ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x=0
Next == IF x=0 THEN x' \in {1,2} ELSE IF x=1 THEN x'=3 ELSE x'=Head(<<>>)
Safe == x<3
====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  },
  {
    name: "error-before-other-parent-violation",
    source: String.raw`---- MODULE Ordered ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x=0
Next == IF x=0 THEN x' \in {1,2} ELSE IF x=1 THEN x'=Head(<<>>) ELSE x'=3
Safe == x<3
====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  },
  {
    name: "same-parent-violation-before-later-error",
    source: String.raw`---- MODULE Ordered ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x=0
Next == IF x=0 THEN x' \in {1,2} ELSE (x'=3 \/ x'=Head(<<>>))
Safe == x<3
====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  },
  {
    name: "shortest-path-before-expensive-longer-path",
    source: String.raw`---- MODULE Shortest ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == IF x=0 THEN x' \in {1,2} ELSE IF x=1 THEN (\A n \in 1..1000: n>0) /\ x'=5 ELSE IF x=2 THEN x'=3 ELSE x'=5
Safe == x<5
====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  },
];
const inputs = [
  ...scalarCases,
  ...collectionCases,
  ...boundaries().filter((input) => !input.expectedRejection),
  ...graphs(seeds.development),
  ...graphs(seeds.confirmation),
  ...cases("small").flatMap((input) => [
    input,
    { ...fault(input), name: `${input.name}-fault` },
  ]),
  ...additional,
];
const comparable = (result: SafetyResult) => ({ ...result, work: 0 });
await mkdir(".artifacts/frontier-safety/controls", { recursive: true });
test("native parallel checks agree with the sequential corpus and preserve ordered failures", async () => {
  const native = startNative();
  try {
    for (const [index, input] of inputs.entries()) {
      const program = compileTlaSafety(input);
      const limits: SafetyBounds =
        "bounds" in input && input.bounds ? input.bounds : bounds;
      const expected = await native.request({
        version: 1,
        request: { kind: "check-safety", program, bounds: limits },
      });
      assert.ok(isRecord(expected));
      for (const workers of [1, 2, 4, 8]) {
        const command = {
          version: 1,
          request: {
            kind: "check-safety-parallel",
            program,
            bounds: limits,
            workers,
          },
        } as const;
        const actual = await native.request(command);
        await writeFile(
          `.artifacts/frontier-safety/controls/${index}-${workers}.json`,
          JSON.stringify({ name: input.name, command, expected, actual }),
        );
        assert.ok(isRecord(actual));
        if (expected.kind === "error")
          assert.deepEqual(actual, expected, input.name);
        else {
          assert.equal(actual.kind, "safety", input.name);
          const reference = decodeSafetyResult(expected.result);
          const result = decodeSafetyResult(actual.result);
          assert.deepEqual(
            comparable(result),
            comparable(reference),
            `${input.name}/${workers}`,
          );
          assert.ok(result.work <= limits.maxWork);
          if (
            workers === 1 ||
            program.actionProperties?.length ||
            program.properties?.length ||
            program.fairness?.length
          )
            assert.deepEqual(result, reference, input.name);
        }
      }
    }
    const small = cases("small")[0];
    assert.ok(small);
    for (const workers of [0, 257, -1, 1.5]) {
      const actual = await native.request({
        version: 1,
        request: {
          kind: "check-safety-parallel",
          program: compileTlaSafety(small),
          bounds,
          workers,
        },
      });
      assert.ok(isRecord(actual));
      assert.equal(actual.kind, "error");
    }
  } finally {
    await native.close();
  }
});

test("the portable Wasm build rejects the native parallel request", async () => {
  const core = await createPortableWasm(
    await readFile(
      "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
    ),
  );
  const small = cases("small")[0];
  assert.ok(small);
  assert.throws(
    () =>
      core.request({
        version: 1,
        request: {
          kind: "check-safety-parallel",
          program: compileTlaSafety(small),
          bounds,
          workers: 2,
        },
      }),
    PortableCoreError,
  );
});
