import assert from "node:assert/strict";
import { test } from "node:test";
import { summarizeStacks } from "../../evals/experiments/profile-tuning/hotspots.js";

test("self weights belong to leaves and recursion is counted once inclusively", () => {
  const result = summarizeStacks([
    { frames: ["root", "a", "a"], weight: 2 },
    { frames: ["root", "a", "b"], weight: 1 },
    { frames: ["root", "b"], weight: 3 },
  ]);
  assert.equal(result.totalWeight, 6);
  assert.deepEqual(
    result.hotspots.find((row) => row.name === "a"),
    { name: "a", self: 2, inclusive: 3 },
  );
  assert.deepEqual(
    result.hotspots.find((row) => row.name === "b"),
    { name: "b", self: 4, inclusive: 4 },
  );
  assert.deepEqual(
    result.hotspots.find((row) => row.name === "root"),
    { name: "root", self: 0, inclusive: 6 },
  );
});
test("empty and invalid sampling data never produce a passing summary", () => {
  assert.throws(() => summarizeStacks([]));
  assert.throws(() => summarizeStacks([{ frames: [], weight: 1 }]));
  assert.throws(() => summarizeStacks([{ frames: ["root"], weight: -1 }]));
});
