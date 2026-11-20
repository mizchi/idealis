import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyProgram } from "../../packages/idealis/src/safety-program.js";
import {
  checkTlaSource,
  compileTlaSource,
} from "../../packages/idealis/src/tla-source.js";

const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const config = "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE";
const bounds = { maxDepth: 100, maxStates: 1000, maxWork: 1000000 };
test("public source wrappers round-trip marked recursive definitions through the Wasm decoder", () => {
  const source =
    "---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\nRECURSIVE Factorial(_)\nFactorial(value) == IF value=0 THEN 1 ELSE value*Factorial(value-1)\nInit == x=Factorial(5)\nNext == UNCHANGED x\nSafe == x=120\n====";
  const program = compileTlaSource(core, { source, config, irVersion: 2 });
  assert.ok(program.version === 2);
  assert.ok(
    program.definitions.some((definition) => definition.recursive === true),
  );
  assert.throws(
    () =>
      decodeSafetyProgram({
        ...program,
        definitions: program.definitions.map((definition) => ({
          ...definition,
          recursive: "true",
        })),
      }),
    PortableCoreError,
  );
  assert.equal(
    checkTlaSource(core, { source, config, irVersion: 2 }, bounds).status,
    "complete",
  );
});
test("public source wrappers preserve recursion evaluation exhaustion as inconclusive", () => {
  const source =
    "---- MODULE Test ----\nVARIABLE x\nRECURSIVE Loop(_)\nLoop(value) == Loop(value)\nInit == x=Loop(0)\nNext == UNCHANGED x\nSafe == TRUE\n====";
  const result = checkTlaSource(core, { source, config, irVersion: 2 }, bounds);
  assert.equal(result.status, "bounded");
  assert.equal(result.reason, "evaluation");
  assert.equal(result.violation, null);
  const tiny = checkTlaSource(
    core,
    { source, config, irVersion: 2 },
    { ...bounds, maxWork: 7 },
  );
  assert.equal(tiny.status, "bounded");
  assert.equal(tiny.reason, "work");
  assert.equal(tiny.work, 7);
});
