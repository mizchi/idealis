import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { after, test } from "node:test";
import {
  bounds,
  config,
  names,
} from "../../evals/experiments/quint-generated/cases.js";
import {
  counter,
  partialOrderCorpus,
} from "../../evals/experiments/symmetry/cases.js";
import { startNative } from "../../evals/experiments/usable/native.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyProgram } from "../../packages/idealis/src/safety-program.js";
import {
  checkSafety,
  decodeSafetyResult,
} from "../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import {
  checkTlaSource,
  compileTlaSource,
} from "../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const native = startNative();
after(() => native.close());

for (const name of names)
  for (const property of ["safety", "broken"] as const)
    test(`Quint v2 preserves v1 behavior: ${name}/${property}`, async () => {
      const input = {
        source: await readFile(
          `fixtures/quint-generated/generated/${name}.tla`,
          "utf8",
        ),
        config: config(property),
      };
      const v1 = checkTlaSource(core, input, bounds);
      const v2 = checkTlaSource(core, { ...input, irVersion: 2 }, bounds);
      assert.deepEqual({ ...v2, work: v1.work }, v1);
      assert.equal(
        v2.status,
        property === "safety" ? "complete" : "invariant-violation",
      );
      for (const irVersion of [1, 2] as const)
        assert.throws(
          () => compileTlaSafety({ ...input, irVersion }),
          /Unsupported TLA input: modules or EXTENDS/,
          "The TS frontend already rejects Quint's TLC/Variants imports in v1",
        );
      for (const program of [
        compileTlaSource(core, { ...input, irVersion: 2 }),
      ]) {
        assert.equal(program.version, 2);
        assert.deepEqual(checkSafety(core, program, bounds), v2);
        for (const workers of [1, 8]) {
          const response = await native.request({
            version: 1,
            request: {
              kind: "check-safety",
              program,
              bounds,
              options: { workers },
            },
          });
          assert.ok(isRecord(response) && response.kind === "safety");
          assert.deepEqual(decodeSafetyResult(response.result), v2);
        }
      }
    });

test("v2 explicitly rejects SYMMETRY and preserves the v1 configuration", () => {
  const input = counter(3, 2);
  const program = compileTlaSource(core, input);
  assert.ok(program.symmetry);
  assert.equal(checkSafety(core, program, bounds).status, "complete");
  assert.throws(
    () => compileTlaSource(core, { ...input, irVersion: 2 }),
    /safety IR v2 does not support SYMMETRY/,
  );
  assert.throws(
    () =>
      checkSafety(
        core,
        decodeSafetyProgram({ ...program, version: 2, definitions: [] }),
        bounds,
      ),
    PortableCoreError,
  );
});

for (const irVersion of [1, 2] as const)
  test(`unmodified TCommit matches TLC with IR v${irVersion}`, async () => {
    const input = {
      source: await readFile("fixtures/tcommit/TCommit.tla", "utf8"),
      config: await readFile("fixtures/tcommit/TCommit.cfg", "utf8"),
      irVersion,
    };
    const program = compileTlaSource(core, input);
    const modelBounds = { ...bounds, maxWork: 1000000000 };
    const result = checkSafety(core, program, modelBounds);
    assert.equal(result.status, "complete");
    assert.equal(result.states, 6816);
    assert.equal(result.generated, 53513);
    for (const workers of [1, 8]) {
      const response = await native.request({
        version: 1,
        request: {
          kind: "check-safety",
          program,
          bounds: modelBounds,
          options: { workers },
        },
      });
      assert.ok(isRecord(response) && response.kind === "safety");
      const actual = decodeSafetyResult(response.result);
      assert.equal(actual.status, result.status);
      assert.equal(actual.states, result.states);
      assert.equal(actual.generated, result.generated);
    }
  });

test("v2 lexical frames make POR fall back to the complete graph", () => {
  const input = partialOrderCorpus()[0];
  assert.ok(input);
  const program = compileTlaSource(core, { ...input, irVersion: 2 });
  assert.equal(program.version, 2);
  const plain = checkSafety(core, program, bounds);
  assert.equal(plain.status, "complete");
  assert.deepEqual(
    checkSafety(core, program, bounds, { partialOrder: true }),
    plain,
  );
});

test("v2 IR and source lowering execute callbacks and Assert", async () => {
  const input = {
    source: `---- MODULE CallbackIR ----
EXTENDS Naturals, Integers, Sequences, TLC
VARIABLE x
Positive(n) == n > 0
Before(a,b) == a < b
Init == x = 0
Next == UNCHANGED x
Safe == SelectSeq(<<-1,1,0,2>>, Positive) = <<1,2>> /\\ SortSeq(<<2,1>>, Before) = <<1,2>> /\\ Assert(TRUE, "checked")
====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
  };
  const compiled = compileTlaSource(core, input);
  const program = decodeSafetyProgram({
    ...compiled,
    version: 2,
    definitions: [],
  });
  const result = checkSafety(core, program, bounds);
  assert.equal(result.status, "complete");
  assert.equal(result.states, 1);
  assert.equal(result.generated, 2);
  const response = await native.request({
    version: 1,
    request: { kind: "check-safety", program, bounds },
  });
  assert.ok(isRecord(response) && response.kind === "safety");
  assert.deepEqual(decodeSafetyResult(response.result), result);
  const sourceProgram = compileTlaSource(core, { ...input, irVersion: 2 });
  assert.equal(sourceProgram.version, 2);
  const sourceResult = checkSafety(core, sourceProgram, bounds);
  assert.equal(sourceResult.status, "complete");
  assert.equal(sourceResult.states, result.states);
  assert.equal(sourceResult.generated, result.generated);
  const sourceResponse = await native.request({
    version: 1,
    request: { kind: "check-safety", program: sourceProgram, bounds },
  });
  assert.deepEqual(sourceResponse, { kind: "safety", result: sourceResult });
});
