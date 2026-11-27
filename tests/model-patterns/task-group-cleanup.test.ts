import assert from "node:assert/strict";
import test from "node:test";
import { setImmediate } from "node:timers/promises";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";
import { taskGate } from "./helpers/task-gate.js";

type Child = "Left" | "Right";
type Command =
  | { readonly kind: "failWork" }
  | { readonly kind: "finish"; readonly child: Child }
  | {
      readonly kind: "exitCleanup";
      readonly child: Child;
      readonly outcome: "Clean" | "Broken";
    };
type Failure = "WorkFailed" | "LeftCleanup" | "RightCleanup";
const children = ["Left", "Right"] as const;
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/cancellation/task-group-cleanup.model";

test("a task group reports the work failure and every finally failure after both children exit", async (t) => {
  const base = await reachableScenario<Command>(
    checker,
    model,
    "BothCleaningAfterWorkFailure",
  );
  assert.deepEqual(base.commands, [{ kind: "failWork" }]);
  for (const fault of [
    "none",
    "drop-cleanup",
    "replace-primary",
    "cleanup-only",
  ] as const) {
    await t.test(fault, async () => {
      const work = { Left: taskGate(), Right: taskGate() };
      const cleanup = { Left: taskGate(), Right: taskGate() };
      const abort = new AbortController();
      const workError = new Error("work failure");
      const cleanupErrors = {
        Left: new Error("left cleanup"),
        Right: new Error("right cleanup"),
      };
      const names = new Map<unknown, Failure>([
        [workError, "WorkFailed"],
        [cleanupErrors.Left, "LeftCleanup"],
        [cleanupErrors.Right, "RightCleanup"],
      ]);
      const stages: Record<Child, "Running" | "Cleaning" | "Exited"> = {
        Left: "Running",
        Right: "Running",
      };
      const failures = new Set<Error>();
      let primary: Error | undefined;
      let result: "Open" | "Succeeded" | "Rejected" = "Open";
      let report: { primary: Failure | null; cleanup: Failure[] } | null = null;
      abort.signal.addEventListener(
        "abort",
        () => work.Right.reject(abort.signal.reason),
        { once: true },
      );
      const tasks = children.map((child) =>
        work[child].promise
          .catch((error: unknown) => {
            if (error === workError) {
              primary = workError;
              if (stages.Right === "Running") abort.abort();
            }
            throw error;
          })
          .finally(async () => {
            stages[child] = "Cleaning";
            try {
              await cleanup[child].promise;
            } catch (error) {
              assert.equal(error, cleanupErrors[child]);
              failures.add(cleanupErrors[child]);
              throw error;
            } finally {
              stages[child] = "Exited";
            }
          }),
      );
      const settled = Promise.allSettled(tasks)
        .then(() => {
          if (!primary && failures.size === 0) return;
          const cause =
            fault === "replace-primary" ? [...failures][0] : primary;
          const details = fault === "drop-cleanup" ? [] : [...failures];
          throw new AggregateError(details, "task group failed", { cause });
        })
        .then(
          () => {
            result = "Succeeded";
          },
          (error: unknown) => {
            assert.ok(error instanceof AggregateError);
            const cleanup = error.errors
              .map((error: unknown) => {
                const name = names.get(error);
                assert.ok(name);
                return name;
              })
              .sort();
            report = { primary: names.get(error.cause) ?? null, cleanup };
            result = "Rejected";
          },
        );
      const commands: Command[] = [
        ...(fault === "cleanup-only"
          ? ([
              { kind: "finish", child: "Left" },
              { kind: "finish", child: "Right" },
            ] as const)
          : base.commands),
        { kind: "exitCleanup", child: "Right", outcome: "Broken" },
        {
          kind: "exitCleanup",
          child: "Left",
          outcome: fault === "cleanup-only" ? "Clean" : "Broken",
        },
      ];
      try {
        const verdict = await recordReplay<Command>(
          checker,
          { ...base, commands },
          {
            async apply(command) {
              if (command.kind === "failWork") work.Left.reject(workError);
              else if (command.kind === "finish") work[command.child].open();
              else if (command.outcome === "Broken")
                cleanup[command.child].reject(cleanupErrors[command.child]);
              else cleanup[command.child].open();
              await setImmediate();
            },
            async observe() {
              return { stages: { ...stages }, result, report };
            },
            project: ({
              stages,
              result,
              publishedPrimary,
              publishedErrors,
            }) => ({
              stages,
              result,
              report:
                result === "Rejected"
                  ? {
                      primary: publishedPrimary,
                      cleanup: [...(publishedErrors as Failure[])].sort(),
                    }
                  : null,
            }),
          },
        );
        if (fault === "none" || fault === "cleanup-only") {
          assert.equal(verdict.verdict, "matched");
          assert.equal(
            (verdict.recording.observations.at(-2) as { result: string })
              .result,
            "Open",
          );
          assert.deepEqual(
            report,
            fault === "cleanup-only"
              ? { primary: null, cleanup: ["RightCleanup"] }
              : {
                  primary: "WorkFailed",
                  cleanup: ["LeftCleanup", "RightCleanup"],
                },
          );
        } else {
          assert.equal(verdict.verdict, "mismatch");
          assert.equal(verdict.difference.step, commands.length);
          assert.deepEqual(
            report,
            fault === "drop-cleanup"
              ? { primary: "WorkFailed", cleanup: [] }
              : {
                  primary: "RightCleanup",
                  cleanup: ["LeftCleanup", "RightCleanup"],
                },
          );
        }
      } finally {
        for (const child of children) {
          work[child].open();
          cleanup[child].open();
        }
        await settled;
      }
    });
  }
});
