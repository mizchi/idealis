import assert from "node:assert/strict";
import { test } from "node:test";
import { cellFrom } from "../../evals/experiments/nesting-chains/stack-harness/cell.js";

test("a spawn failure is recorded with absent streams", () => {
  const cell = cellFrom("if", 3, {
    status: null,
    signal: null,
    error: new Error("spawn ENOENT"),
    stdout: null,
    stderr: undefined,
  });
  assert.equal(cell.spawnError, "Error: spawn ENOENT");
  assert.equal(cell.stdout, "");
  assert.equal(cell.stderr, "");
  assert.equal(cell.exit, null);
});

test("a completed run keeps trimmed streams and no spawn error", () => {
  const cell = cellFrom("flat", 47, {
    status: 0,
    signal: null,
    stdout: " ok\n",
    stderr: "\n",
  });
  assert.equal(cell.spawnError, null);
  assert.equal(cell.stdout, "ok");
  assert.equal(cell.exit, 0);
});
