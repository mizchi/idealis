import assert from "node:assert/strict";
import { test } from "node:test";
import {
  executableDigest,
  withNames,
} from "../../evals/experiments/profile-tuning/wasm-names.js";

const program = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 127, 3, 2, 1, 0, 7, 5, 1, 1,
  102, 0, 0, 10, 6, 1, 4, 0, 65, 42, 11,
]);
const names = new Uint8Array([
  0, 16, 4, 110, 97, 109, 101, 1, 9, 1, 0, 6, 97, 110, 115, 119, 101, 114,
]);

test("name injection preserves executable bytes and function behavior", () => {
  const named = withNames(program, new Uint8Array([...program, ...names]));
  assert.deepEqual(named.subarray(0, program.length), program);
  assert.equal(executableDigest(named), executableDigest(program));
  const module = new WebAssembly.Module(named);
  assert.equal(WebAssembly.Module.customSections(module, "name").length, 1);
  const instance = new WebAssembly.Instance(module);
  const answer = instance.exports.f;
  assert.ok(typeof answer === "function");
  assert.equal(answer(), 42);
});

test("a changed function body cannot borrow the old names", () => {
  const changed = program.slice();
  changed[changed.length - 2] = 43;
  assert.notEqual(executableDigest(changed), executableDigest(program));
  assert.throws(
    () => withNames(changed, new Uint8Array([...program, ...names])),
    /Function code differs/,
  );
});

test("invalid modules and absent names fail before recording", () => {
  assert.throws(() => withNames(program, program), /Missing name section/);
  assert.throws(
    () => executableDigest(program.subarray(0, program.length - 1)),
    /Invalid Wasm/,
  );
});

test("function imports cannot change the meaning of function name indices", () => {
  const imported = new Uint8Array([
    ...program.subarray(0, 15),
    2,
    7,
    1,
    1,
    101,
    1,
    120,
    0,
    0,
    ...program.subarray(15),
    ...names,
  ]);
  assert.ok(WebAssembly.validate(imported));
  assert.throws(() => withNames(program, imported), /Function code differs/);
});
