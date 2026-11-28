import assert from "node:assert/strict";
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
import { taskGate } from "./helpers/task-gate.js";

type Leave = "BreakLoop" | "ThrowBody";
type Cleanup = "Succeed" | "Reject" | "NonObject";
type Result = "Normal" | "BodyError" | "CleanupError" | "InvalidReturn";
type Command =
  | { kind: "leave"; reason: Leave }
  | { kind: "finishClose"; result: Cleanup };

const checker = "crates/idealis-check/target/debug/idealis-check";
const model =
  "examples/model-patterns/node-event-loop/async-iterator-close.ideal";

function openIterator(manualClose = false) {
  const returning = taskGate();
  const release = taskGate();
  const bodyError = new Error("loop body failed");
  const cleanupError = new Error("return failed");
  let calls = 0;
  let requestedCleanup: Cleanup | null = null;
  let cleanup: Cleanup | null = null;
  let leaveReason: Leave | null = null;
  let outcome: Result | null = null;
  let execution: Promise<void> | null = null;

  const iterator = {
    [Symbol.asyncIterator]() {
      return this;
    },
    async next() {
      return { done: false as const, value: 1 };
    },
    async return(): Promise<IteratorResult<number, undefined>> {
      calls++;
      assert.equal(calls, 1);
      returning.open();
      await release.promise;
      assert.ok(requestedCleanup);
      cleanup = requestedCleanup;
      if (cleanup === "Reject") throw cleanupError;
      if (cleanup === "NonObject") {
        // Deliberately violate the iterator protocol to exercise AsyncIteratorClose.
        return 0 as unknown as IteratorResult<number, undefined>;
      }
      return { done: true, value: undefined };
    },
  };

  function body(reason: Leave, value: number) {
    assert.equal(value, 1);
    leaveReason = reason;
    if (reason === "ThrowBody") throw bodyError;
  }

  async function nativeLoop(reason: Leave) {
    for await (const value of iterator) {
      body(reason, value);
      break;
    }
  }

  async function manualLoop(reason: Leave) {
    const { value } = await iterator.next();
    try {
      body(reason, value);
    } finally {
      await iterator.return();
    }
  }

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "leave": {
          const loop = manualClose ? manualLoop : nativeLoop;
          execution = loop(command.reason).then(
            () => {
              outcome = "Normal";
            },
            (error: unknown) => {
              if (error === bodyError) outcome = "BodyError";
              else if (error === cleanupError) outcome = "CleanupError";
              else {
                assert.ok(error instanceof TypeError);
                outcome = "InvalidReturn";
              }
            },
          );
          void execution.catch(() => undefined);
          await returning.promise;
          await setImmediate();
          break;
        }
        case "finishClose":
          requestedCleanup = command.result;
          release.open();
          assert.ok(execution);
          await execution;
          break;
      }
    },
    async observe() {
      const phase =
        outcome !== null ? "Finished" : calls === 0 ? "Idle" : "Closing";
      return { phase, leaveReason, cleanup, outcome };
    },
    project: (snapshot: Readonly<Record<string, unknown>>) => snapshot,
  };
}

test(
  "for-await awaits return and preserves a body throw over a cleanup failure",
  { timeout: 10_000 },
  async (t) => {
    const cases = [
      {
        reason: "BreakLoop",
        question: "ClosingAfterBreak",
        expected: ["Normal", "CleanupError", "InvalidReturn"],
      },
      {
        reason: "ThrowBody",
        question: "ClosingAfterThrow",
        expected: ["BodyError", "BodyError", "BodyError"],
      },
    ] as const;
    const cleanupResults = ["Succeed", "Reject", "NonObject"] as const;
    for (const row of cases) {
      const base = await reachableScenario<Command>(
        checker,
        model,
        row.question,
        { checker: "rprob" },
      );
      for (const [index, result] of cleanupResults.entries()) {
        await t.test(`${row.reason} / ${result}`, async () => {
          const driver = openIterator();
          const replay = await recordReplay(
            checker,
            {
              ...base,
              commands: [...base.commands, { kind: "finishClose", result }],
            },
            driver,
          );
          assert.equal(replay.verdict, "matched");
          assert.deepEqual(replay.recording.observations.at(-2), {
            phase: "Closing",
            leaveReason: row.reason,
            cleanup: null,
            outcome: null,
          });
          assert.deepEqual(replay.recording.observations.at(-1), {
            phase: "Finished",
            leaveReason: row.reason,
            cleanup: result,
            outcome: row.expected[index],
          });
        });
      }
    }
    for (const fault of [
      {
        question: "ClosingAfterThrow",
        result: "Reject",
        expected: "BodyError",
        observed: "CleanupError",
      },
      {
        question: "ClosingAfterBreak",
        result: "NonObject",
        expected: "InvalidReturn",
        observed: "Normal",
      },
    ] as const) {
      await t.test(
        `manual finally / ${fault.question} / ${fault.result}`,
        async () => {
          const driver = openIterator(true);
          const base = await reachableScenario<Command>(
            checker,
            model,
            fault.question,
            { checker: "rprob" },
          );
          const replay = await recordReplay(
            checker,
            {
              ...base,
              commands: [
                ...base.commands,
                { kind: "finishClose", result: fault.result },
              ],
            },
            driver,
          );
          assert.equal(replay.verdict, "mismatch");
          assert.equal(replay.difference.step, 2);
          assert.partialDeepStrictEqual(replay.difference.expected, {
            outcome: fault.expected,
          });
          assert.partialDeepStrictEqual(replay.difference.observed, {
            outcome: fault.observed,
          });
          const itf = JSON.parse(JSON.stringify(toReplayItf(replay.recording)));
          assert.deepEqual(
            await replayRecording(checker, fromReplayItf(itf), driver.project),
            {
              verdict: "mismatch",
              difference: replay.difference,
            },
          );
        },
      );
    }
  },
);
