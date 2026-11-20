import assert from "node:assert/strict";
import { test } from "node:test";
import { cases } from "../../evals/experiments/apalache-comparison/cases.js";
import {
  classify,
  itfStates,
  validateWitness,
} from "../../evals/experiments/apalache-comparison/results.js";

test("bounded success, counterexample, solver failure and missing results stay distinct", () => {
  assert.equal(
    classify(
      0,
      "Checker reports no error up to computation length 4\nThe outcome is: NoError",
    ),
    "bounded-no-counterexample",
  );
  assert.equal(
    classify(12, "State 1: state invariant 0 violated.\nThe outcome is: Error"),
    "counterexample",
  );
  for (const [code, log] of [
    [0, ""],
    [0, "The outcome is: Unknown"],
    [1, "The outcome is: NoError"],
  ] as const)
    assert.equal(classify(code, log), "error");
});
test("ITF and independent trace checks reject invalid initial states, transitions and non-violations", () => {
  const input = cases.find((entry) => entry.name === "ring-fault-8");
  assert.ok(input);
  const states = Array.from({ length: 9 }, (_, index) => index);
  validateWitness(input, states);
  assert.throws(() => validateWitness(input, states.slice(1)));
  assert.throws(() => validateWitness(input, [0, 8]));
  assert.throws(() => validateWitness(input, [0, 1]));
  assert.deepEqual(
    itfStates(
      {
        vars: ["x"],
        states: [{ x: { "#bigint": "0" } }, { x: { "#bigint": "1" } }],
      },
      "ring",
    ),
    [0, 1],
  );
  assert.throws(() =>
    itfStates({ vars: ["x"], states: [{ x: { "#bigint": "01" } }] }, "ring"),
  );
});
