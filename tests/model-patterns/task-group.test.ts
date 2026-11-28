import assert from "node:assert/strict";
import test from "node:test";
import { setImmediate } from "node:timers/promises";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";
import { taskGate } from "./helpers/task-gate.js";

type Child = "Left" | "Right";
type Command = {
  readonly kind: "fail" | "finish" | "exitCleanup";
  readonly child: Child;
};
type Stage = "Running" | "Cleaning" | "Exited";
const children = ["Left", "Right"] as const;
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/cancellation/task-group.ideal";

test("a task group cancels siblings and waits for their actual finally blocks", async (t) => {
  const base = await reachableScenario<Command>(
    checker,
    model,
    "LeftFailedWhileSiblingIsCleaning",
  );
  assert.deepEqual(base.commands, [{ kind: "fail", child: "Left" }]);
  const commands: Command[] = [
    ...base.commands,
    { kind: "exitCleanup", child: "Left" },
    { kind: "exitCleanup", child: "Right" },
  ];
  for (const fault of [
    "none",
    "early-return",
    "no-cancellation",
    "success",
  ] as const) {
    await t.test(fault, async () => {
      const stages: Record<Child, Stage> = {
        Left: "Running",
        Right: "Running",
      };
      const abort = {
        Left: new AbortController(),
        Right: new AbortController(),
      };
      const work = { Left: taskGate(), Right: taskGate() };
      const cleanup = { Left: taskGate(), Right: taskGate() };
      let failedBy: Child | null = null;
      let parent: "Open" | "Succeeded" | "Rejected" = "Open";
      let returnedFailure: Child | null = null;
      const failures = {
        Left: new Error("left failed"),
        Right: new Error("right failed"),
      };

      const tasks = children.map(async (child) => {
        abort[child].signal.addEventListener(
          "abort",
          () => work[child].reject(abort[child].signal.reason),
          { once: true },
        );
        try {
          await work[child].promise;
        } catch (error) {
          if (!abort[child].signal.aborted && failedBy === null) {
            failedBy = child;
            if (fault !== "no-cancellation") {
              for (const sibling of children) {
                if (sibling !== child && stages[sibling] === "Running")
                  abort[sibling].abort();
              }
            }
          }
          throw error;
        } finally {
          stages[child] = "Cleaning";
          await cleanup[child].promise;
          stages[child] = "Exited";
        }
      });
      const group =
        fault === "early-return"
          ? Promise.all(tasks)
          : Promise.allSettled(tasks).then(() => {
              if (failedBy !== null) throw failures[failedBy];
            });
      const completion = group.then(
        () => {
          parent = "Succeeded";
        },
        (error: unknown) => {
          const child = children.find(
            (candidate) => error === failures[candidate],
          );
          assert.ok(child, "the parent must return the child's original error");
          returnedFailure = child;
          parent = "Rejected";
        },
      );
      const run: Command[] =
        fault === "success"
          ? [
              { kind: "finish", child: "Left" },
              { kind: "exitCleanup", child: "Left" },
              { kind: "finish", child: "Right" },
              { kind: "exitCleanup", child: "Right" },
            ]
          : commands;
      try {
        const result = await recordReplay<Command>(
          checker,
          { ...base, commands: run },
          {
            async apply({ kind, child }) {
              if (kind === "fail") work[child].reject(failures[child]);
              else if (kind === "finish") work[child].open();
              else cleanup[child].open();
              await setImmediate();
            },
            async observe() {
              return {
                stages: { ...stages },
                failedBy,
                parent,
                returnedFailure,
                signalled: children.filter(
                  (child) => abort[child].signal.aborted,
                ),
              };
            },
            project: ({ stages, failedBy, parent, signalled }) => ({
              stages,
              failedBy,
              parent,
              returnedFailure: parent === "Rejected" ? failedBy : null,
              signalled,
            }),
          },
        );
        if (fault === "none" || fault === "success") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations.at(-1), {
            stages: { Left: "Exited", Right: "Exited" },
            failedBy: fault === "success" ? null : "Left",
            parent: fault === "success" ? "Succeeded" : "Rejected",
            returnedFailure: fault === "success" ? null : "Left",
            signalled: fault === "success" ? [] : ["Right"],
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(
            result.difference.step,
            fault === "early-return" ? 2 : 1,
          );
          assert.deepEqual(
            result.difference.observed,
            fault === "early-return"
              ? {
                  stages: { Left: "Exited", Right: "Cleaning" },
                  failedBy: "Left",
                  parent: "Rejected",
                  returnedFailure: "Left",
                  signalled: ["Right"],
                }
              : {
                  stages: { Left: "Cleaning", Right: "Running" },
                  failedBy: "Left",
                  parent: "Open",
                  returnedFailure: null,
                  signalled: [],
                },
          );
        }
      } finally {
        for (const child of children) {
          work[child].open();
          cleanup[child].open();
        }
        await completion;
        await Promise.allSettled(tasks);
      }
    });
  }
});
