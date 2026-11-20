import assert from "node:assert/strict";
import { test } from "node:test";
import { cases, fault } from "../../evals/experiments/single-model/cases.js";
import { createPhaseRecorder } from "../../evals/experiments/single-model/phases.js";

test("wide benchmark sizes have independently known closed-graph counts", () => {
  assert.deepEqual(
    cases("large").map((input) => [
      input.name,
      input.states,
      input.generated,
      input.initialStates,
    ]),
    [
      ["counter-product", 5308416, 21233665, 1],
      ["function-cube", 1048576, 20971521, 1],
      ["parallel-channels", 2097152, 22020097, 1],
      ["linear-ring", 1000000, 1000001, 1],
    ],
  );
  assert.equal(cases("large")[0]?.layerWidth, 73744);
  assert.equal(cases("large")[1]?.layerWidth, 184756);
  assert.ok((cases("large")[2]?.layerWidth ?? 0) > 100000);
  assert.equal(cases("large")[3]?.layerWidth, 1);
});
test("small models and fault controls retain the same initial and next definitions", () => {
  assert.deepEqual(
    cases("small").map((input) => [
      input.states,
      input.generated,
      input.layerWidth,
    ]),
    [
      [81, 325, 19],
      [16, 65, 6],
      [64, 193, 14],
      [17, 18, 1],
    ],
  );
  for (const input of cases("small")) {
    const broken = fault(input);
    assert.equal(
      input.source.slice(0, input.source.indexOf("Safe ==")),
      broken.source.slice(0, broken.source.indexOf("Safe ==")),
    );
    assert.match(broken.config, /INVARIANT Safe/);
  }
});
test("phase markers handle split stream chunks and reject missing or repeated boundaries", () => {
  const recorder = createPhaseRecorder();
  recorder.add("@!@!@STARTM", 10);
  recorder.add("SG 2189:0 @!@!@\nComputing initial states...\n", 12);
  recorder.add(
    "@!@!@STARTMSG 2190:0 @!@!@\nFinished computing initial states: 1 distinct state generated\n",
    14,
  );
  recorder.add("@!@!@STARTMSG 2193:0 @!@!@\n", 25);
  assert.deepEqual(recorder.finish(), {
    initStartMs: 12,
    initDoneMs: 14,
    successMs: 25,
    initAndSearchMs: 13,
    searchAndFinalMs: 11,
  });
  assert.throws(() => createPhaseRecorder().finish());
  const repeated = createPhaseRecorder();
  repeated.add("@!@!@STARTMSG 2189:0 @!@!@\n@!@!@STARTMSG 2189:0 @!@!@\n", 1);
  assert.throws(() => repeated.finish());
});
