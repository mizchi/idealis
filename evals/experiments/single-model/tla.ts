import assert from "node:assert/strict";
import { isRecord } from "../../../packages/idealis/src/trace.js";

export const normalizeTla = (
  value: unknown,
  expected: { readonly states: number; readonly generated: number },
  status: "complete" | "invariant-violation",
) => {
  assert.ok(isRecord(value));
  assert.equal(value.status, status === "complete" ? "success" : status);
  assert.ok(
    typeof value.distinctStates === "number" &&
      Number.isSafeInteger(value.distinctStates) &&
      value.distinctStates >= 0,
  );
  assert.ok(
    typeof value.generatedStates === "number" &&
      Number.isSafeInteger(value.generatedStates) &&
      value.generatedStates >= 0,
  );
  if (status === "complete") {
    assert.equal(value.distinctStates, expected.states);
    assert.equal(value.generatedStates, expected.generated);
    assert.ok(isRecord(value.initialStates));
    assert.equal(value.initialStates.distinct, 1);
    assert.equal(value.initialStates.generated, 1);
    assert.ok(value.violation === undefined || value.violation === null);
  } else {
    assert.ok(
      isRecord(value.violation) &&
        Array.isArray(value.violation.trace) &&
        value.violation.trace.length > 1,
    );
  }
  return {
    status,
    states: value.distinctStates,
    generated: value.generatedStates,
  };
};
