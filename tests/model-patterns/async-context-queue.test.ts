import assert from "node:assert/strict";
import { AsyncLocalStorage } from "node:async_hooks";
import test from "node:test";
import { setImmediate } from "node:timers/promises";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";

type Task = "First" | "Second";
type Request = "Alpha" | "Beta";
type Context = Request | "Dispatcher";
type Outcome = "Completed" | "Failed";
type Command =
  | { kind: "enqueue"; task: Task; context: Request }
  | { kind: "runNext"; outcome: Outcome };

const checker = "crates/rcheck/target/debug/rcheck";
const model =
  "examples/model-patterns/node-event-loop/async-context-queue.model";

function taskResults<Value>(): Record<Task, Value | null> {
  return { First: null, Second: null };
}

function openQueue(capture: boolean) {
  const storage = new AsyncLocalStorage<Context>();
  const submitted = taskResults<Request>();
  const entered = taskResults<Context>();
  const resumed = taskResults<Context>();
  const restored = taskResults<Context>();
  const outcomes = taskResults<Outcome>();
  const pending: {
    task: Task;
    run: (callback: () => Promise<void>) => Promise<void>;
  }[] = [];
  const failed = new Error("work failed after yielding");

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "enqueue":
          storage.run(command.context, () => {
            const observed = storage.getStore();
            assert.ok(observed && observed !== "Dispatcher");
            submitted[command.task] = observed;
            pending.push({
              task: command.task,
              run: capture
                ? AsyncLocalStorage.snapshot()
                : (callback) => callback(),
            });
          });
          break;
        case "runNext":
          await storage.run("Dispatcher", async () => {
            const job = pending.shift();
            assert.ok(job);
            const { task } = job;
            try {
              await job.run(async () => {
                entered[task] = storage.getStore() ?? null;
                await setImmediate();
                resumed[task] = storage.getStore() ?? null;
                if (command.outcome === "Failed") throw failed;
              });
              outcomes[task] = "Completed";
            } catch (error) {
              assert.equal(error, failed);
              outcomes[task] = "Failed";
            }
            restored[task] = storage.getStore() ?? null;
          });
          assert.equal(storage.getStore(), undefined);
          break;
      }
    },
    async observe() {
      return {
        queue: { $sequence: pending.map(({ task }) => task) },
        submitted: { ...submitted },
        entered: { ...entered },
        resumed: { ...resumed },
        restored: { ...restored },
        outcomes: { ...outcomes },
      };
    },
    project: (snapshot: Readonly<Record<string, unknown>>) => snapshot,
  };
}

test("queued async work keeps each submission context and restores the dispatcher after failure", async (t) => {
  const prefix = await reachableScenario<Command>(
    checker,
    model,
    "DistinctQueuedContexts",
    { checker: "rprob" },
  );
  const scenario = {
    ...prefix,
    commands: [
      ...prefix.commands,
      { kind: "runNext", outcome: "Failed" },
      { kind: "runNext", outcome: "Completed" },
    ],
  } satisfies typeof prefix;
  await t.test("each entry and continuation sees its context", async () => {
    const driver = openQueue(true);
    const result = await recordReplay(checker, scenario, driver);
    assert.equal(result.verdict, "matched");
    assert.deepEqual(await driver.observe(), {
      queue: { $sequence: [] },
      submitted: { First: "Alpha", Second: "Beta" },
      entered: { First: "Alpha", Second: "Beta" },
      resumed: { First: "Alpha", Second: "Beta" },
      restored: { First: "Dispatcher", Second: "Dispatcher" },
      outcomes: { First: "Failed", Second: "Completed" },
    });
  });
  await t.test(
    "direct invocation inherits the dispatcher and changes the recorded context",
    async () => {
      const driver = openQueue(false);
      const result = await recordReplay(checker, scenario, driver);
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, prefix.commands.length + 1);
      assert.partialDeepStrictEqual(result.difference.expected, {
        entered: { First: "Alpha", Second: null },
        resumed: { First: "Alpha", Second: null },
      });
      assert.partialDeepStrictEqual(result.difference.observed, {
        entered: { First: "Dispatcher", Second: null },
        resumed: { First: "Dispatcher", Second: null },
        restored: { First: "Dispatcher", Second: null },
        outcomes: { First: "Failed", Second: null },
      });
      const itf = JSON.parse(JSON.stringify(toReplayItf(result.recording)));
      assert.deepEqual(
        await replayRecording(checker, fromReplayItf(itf), driver.project),
        {
          verdict: "mismatch",
          difference: result.difference,
        },
      );
    },
  );
});
