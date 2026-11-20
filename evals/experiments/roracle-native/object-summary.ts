import assert from "node:assert/strict";
import type { SavedFixture } from "./measure.js";

export interface ObjectSample {
  readonly fixture: string;
  readonly stage: string;
  readonly sample: number;
  readonly calls: number;
  readonly totalNs: number;
}

export const summarizeObject = (
  cases: readonly SavedFixture[],
  records: readonly ObjectSample[],
) => {
  assert.ok(cases.length > 0, "Empty speed comparison");
  assert.ok(
    cases.every(
      (fixture) =>
        Number.isSafeInteger(fixture.iterations) && fixture.iterations > 0,
    ),
    "Invalid iteration budget",
  );
  const stages = ["before-object", "after-object", "before-json", "after-json"];
  assert.equal(records.length, cases.length * stages.length * 30);
  const rows = cases.map((fixture) => {
    const timings = stages.map((stage) => {
      const selected = records.filter(
        (row) => row.fixture === fixture.name && row.stage === stage,
      );
      assert.deepEqual(
        selected.map((row) => row.sample),
        Array.from({ length: 30 }, (_, index) => index),
      );
      assert.ok(
        selected.every(
          (row) =>
            row.calls === fixture.iterations &&
            Number.isSafeInteger(row.totalNs) &&
            row.totalNs > 0,
        ),
      );
      const times = selected
        .map((row) => row.totalNs / row.calls)
        .sort((left, right) => left - right);
      const lower = times[14];
      const upper = times[15];
      assert.ok(lower && upper);
      return (lower + upper) / 2000;
    });
    const beforeObject = timings[0];
    const afterObject = timings[1];
    const beforeJson = timings[2];
    const afterJson = timings[3];
    assert.ok(beforeObject && afterObject && beforeJson && afterJson);
    return {
      fixture: fixture.name,
      beforeObject,
      afterObject,
      beforeJson,
      afterJson,
      objectSpeedup: beforeObject / afterObject,
      jsonSpeedup: beforeJson / afterJson,
    };
  });
  const geometricMean = Math.exp(
    rows.reduce((total, row) => total + Math.log(row.objectSpeedup), 0) /
      rows.length,
  );
  const eligible =
    geometricMean >= 1.15 &&
    rows.every(
      (row) => row.objectSpeedup >= 1 / 1.1 && row.jsonSpeedup >= 1 / 1.1,
    );
  return { rows, geometricMean, eligible };
};
