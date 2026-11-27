/// <reference lib="esnext.disposable" />
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

type Cause = "Body" | "Inner" | "Outer";
type Disposal = "Succeeded" | "Failed";
type Stage = "Active" | "ClosingInner" | "ClosingOuter" | "Done";
type Command =
  | { kind: "leave"; result: "Returned" | "Threw" }
  | { kind: "finishInner" | "finishOuter"; result: Disposal };
type Scope = (
  outer: AsyncDisposable,
  inner: AsyncDisposable,
  body: () => void,
) => Promise<void>;

const checker = "crates/rcheck/target/debug/rcheck";
const model =
  "examples/model-patterns/node-event-loop/async-disposal-errors.model";

// Pass this syntax directly to Node instead of lowering it to TS helpers.
const withAwaitUsing = new Function(
  "outer",
  "inner",
  "body",
  `
  return (async () => {
    await using first = outer;
    await using second = inner;
    body();
  })();
`,
) as Scope;

const withManualFinally: Scope = async (outer, inner, body) => {
  try {
    try {
      body();
    } finally {
      await inner[Symbol.asyncDispose]();
    }
  } finally {
    await outer[Symbol.asyncDispose]();
  }
};

function openScope(manualFinally = false) {
  const seen: Cause[] = [];
  const reported: Cause[] = [];
  const errors = {
    Body: new Error("body"),
    Inner: new Error("inner"),
    Outer: new Error("outer"),
  };
  const states = {
    Inner: { entered: taskGate(), release: taskGate() },
    Outer: { entered: taskGate(), release: taskGate() },
  };
  let stage: Stage = "Active";
  let execution: Promise<void> | null = null;

  function causeFor(error: unknown): Cause {
    if (error === errors.Body) return "Body";
    if (error === errors.Inner) return "Inner";
    assert.equal(error, errors.Outer);
    return "Outer";
  }
  function readSuppressed(error: unknown) {
    if (error instanceof SuppressedError) {
      readSuppressed(error.suppressed);
      reported.push(causeFor(error.error));
    } else {
      reported.push(causeFor(error));
    }
  }
  function resource(which: "Inner" | "Outer"): AsyncDisposable {
    const gate = states[which];
    return {
      async [Symbol.asyncDispose]() {
        stage = which === "Inner" ? "ClosingInner" : "ClosingOuter";
        gate.entered.open();
        try {
          await gate.release.promise;
        } catch (error) {
          assert.equal(error, errors[which]);
          seen.push(which);
          throw error;
        }
      },
    };
  }
  function complete(which: "Inner" | "Outer", result: Disposal) {
    const gate = states[which].release;
    if (result === "Succeeded") gate.open();
    else gate.reject(errors[which]);
  }

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "leave": {
          const scope = manualFinally ? withManualFinally : withAwaitUsing;
          execution = scope(resource("Outer"), resource("Inner"), () => {
            if (command.result === "Threw") {
              seen.push("Body");
              throw errors.Body;
            }
          }).then(
            () => {
              stage = "Done";
            },
            (error: unknown) => {
              readSuppressed(error);
              stage = "Done";
            },
          );
          void execution.catch(() => undefined);
          await states.Inner.entered.promise;
          await setImmediate();
          break;
        }
        case "finishInner":
          complete("Inner", command.result);
          await states.Outer.entered.promise;
          await setImmediate();
          break;
        case "finishOuter":
          assert.ok(execution);
          complete("Outer", command.result);
          await execution;
          break;
      }
    },
    async observe() {
      return {
        stage,
        seen: { $sequence: [...seen] },
        reported: { $sequence: [...reported] },
      };
    },
    project: (snapshot: Readonly<Record<string, unknown>>) => snapshot,
  };
}

test(
  "await using runs both disposers in reverse order and retains the suppressed error chain",
  { timeout: 10_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "StartingScope",
      { checker: "rprob" },
    );
    for (const body of ["Returned", "Threw"] as const) {
      for (const inner of ["Succeeded", "Failed"] as const) {
        for (const outer of ["Succeeded", "Failed"] as const) {
          await t.test(`${body} / ${inner} / ${outer}`, async () => {
            const driver = openScope();
            const result = await recordReplay(
              checker,
              {
                ...base,
                commands: [
                  { kind: "leave", result: body },
                  { kind: "finishInner", result: inner },
                  { kind: "finishOuter", result: outer },
                ],
              },
              driver,
            );
            assert.equal(result.verdict, "matched");
          });
        }
      }
    }
    await t.test("nested finally drops both earlier failures", async () => {
      const driver = openScope(true);
      const result = await recordReplay(
        checker,
        {
          ...base,
          commands: [
            { kind: "leave", result: "Threw" },
            { kind: "finishInner", result: "Failed" },
            { kind: "finishOuter", result: "Failed" },
          ],
        },
        driver,
      );
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, 3);
      assert.partialDeepStrictEqual(result.difference.expected, {
        seen: { $sequence: ["Body", "Inner", "Outer"] },
        reported: { $sequence: ["Body", "Inner", "Outer"] },
      });
      assert.partialDeepStrictEqual(result.difference.observed, {
        seen: { $sequence: ["Body", "Inner", "Outer"] },
        reported: { $sequence: ["Outer"] },
      });
      const itf = JSON.parse(JSON.stringify(toReplayItf(result.recording)));
      assert.deepEqual(
        await replayRecording(checker, fromReplayItf(itf), driver.project),
        {
          verdict: "mismatch",
          difference: result.difference,
        },
      );
    });
  },
);
