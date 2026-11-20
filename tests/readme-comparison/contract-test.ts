import assert from "node:assert/strict";
import { test } from "node:test";
import {
  normalizeTla,
  renderComparison,
} from "../../evals/experiments/readme-comparison/protocol.js";

const expected = { states: 16, generated: 65 };
const success = {
  status: "success",
  distinctStates: 16,
  generatedStates: 65,
  initialStates: { distinct: 1, generated: 1 },
};
test("tla-ts must complete with the exact counts and never receives invented work", () => {
  assert.deepEqual(normalizeTla(success, expected, "complete"), {
    status: "complete",
    states: 16,
    generated: 65,
  });
  for (const selected of [
    { ...success, status: "error" },
    { ...success, status: "bounded" },
    { ...success, distinctStates: undefined },
    { ...success, distinctStates: 15 },
    { ...success, generatedStates: 64 },
    { ...success, initialStates: { distinct: 2, generated: 2 } },
  ])
    assert.throws(() => normalizeTla(selected, expected, "complete"));
  assert.throws(() =>
    normalizeTla(
      { ...success, status: "invariant-violation", violation: { trace: [] } },
      expected,
      "invariant-violation",
    ),
  );
});

test("README comparisons reject missing routes and incomplete results", () => {
  assert.throws(() => renderComparison({ status: "complete", summaries: [] }));
  assert.throws(() =>
    renderComparison({ status: "incomplete", summaries: [] }),
  );
});
