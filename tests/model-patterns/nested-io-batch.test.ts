import assert from "node:assert/strict";
import { readFile } from "node:fs";
import test from "node:test";
import {
  type DrainIoBatch,
  ioBatchAdapter,
} from "../../examples/model-patterns/node-event-loop/adapter.js";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/node-event-loop/nested-io-batch.ideal";
type Callback = "Microtask" | "NestedMicrotask" | "Immediate" | "Timer";

test("a nested microtask fits the modeled I/O batch without losing its order", async () => {
  const scenario = await reachableScenario<DrainIoBatch>(
    checker,
    model,
    "ObservedBatch",
    { checker: "rprob" },
  );
  const reference = await reachableScenario<DrainIoBatch>(
    checker,
    model,
    "ObservedBatch",
  );
  assert.deepEqual(scenario, reference);
  assert.equal(scenario.commands.length, 1);
  for (const deferNested of [false, true]) {
    const calls: Callback[] = [];
    const adapter = ioBatchAdapter({
      async drain() {
        // The callback API keeps the scheduling origin in the I/O callback.
        await new Promise<void>((resolve, reject) => {
          readFile(new URL(import.meta.url), (error) => {
            if (error) {
              reject(error);
              return;
            }
            const record = (name: Callback) => {
              calls.push(name);
              if (calls.length === 4) resolve();
            };
            queueMicrotask(() => {
              record("Microtask");
              if (!deferNested) queueMicrotask(() => record("NestedMicrotask"));
            });
            setImmediate(() => {
              record("Immediate");
              if (deferNested) queueMicrotask(() => record("NestedMicrotask"));
            });
            setTimeout(() => record("Timer"), 0);
          });
        });
      },
      observe: () => calls,
    });
    const result = await recordReplay(checker, scenario, adapter);
    const expected = {
      $sequence: ["Microtask", "NestedMicrotask", "Immediate", "Timer"],
    };
    if (deferNested) {
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, 1);
      assert.deepEqual(result.difference.expected, expected);
      assert.deepEqual(result.difference.observed, {
        $sequence: ["Microtask", "Immediate", "NestedMicrotask", "Timer"],
      });
    } else {
      assert.equal(result.verdict, "matched");
      assert.deepEqual(result.recording.observations, [
        { $sequence: [] },
        expected,
      ]);
    }
  }
});
