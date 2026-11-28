import assert from "node:assert/strict";
import test from "node:test";
import { setImmediate } from "node:timers/promises";
import {
  reachableScenario,
  recordReplay,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";
import { taskGate } from "./helpers/task-gate.js";

type Child = "Left" | "Right";
type Reason =
  | "ParentCancelled"
  | "LeftFailed"
  | "RightFailed"
  | "ChildCancelled";
type Command =
  | { readonly kind: "cancelParent" }
  | {
      readonly kind: "fail" | "acceptCancel" | "exitCleanup";
      readonly child: Child;
    };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/cancellation/task-group-cause.model";
const children = ["Left", "Right"] as const;

test("a task group retains the first cause across parent cancellation and child failure", async (t) => {
  const cancelFirst = await reachableScenario<Command>(
    checker,
    model,
    "ParentCancelledBeforeChildFailure",
  );
  const failFirst = await reachableScenario<Command>(
    checker,
    model,
    "ChildFailedBeforeParentCancellation",
  );
  const cases = [
    {
      name: "cancel first",
      base: cancelFirst,
      next: { kind: "fail", child: "Left" },
      expected: "ParentCancelled",
      overwritten: "LeftFailed",
    },
    {
      name: "failure first",
      base: failFirst,
      next: { kind: "cancelParent" },
      expected: "LeftFailed",
      overwritten: "ParentCancelled",
    },
  ] as const;

  for (const scenario of cases) {
    assert.equal(scenario.base.commands.length, 1);
    for (const overwrite of [false, true]) {
      await t.test(
        `${scenario.name} / ${overwrite ? "overwrite cause" : "keep cause"}`,
        async () => {
          const controllers = {
            Left: new AbortController(),
            Right: new AbortController(),
          };
          const parent = new AbortController();
          const stages = { Left: "Running", Right: "Running" } as Record<
            Child,
            "Running" | "Cleaning" | "Exited"
          >;
          const reported: Record<Child, Reason | null> = {
            Left: null,
            Right: null,
          };
          const work = { Left: taskGate(), Right: taskGate() };
          const cleanup = { Left: taskGate(), Right: taskGate() };
          const parentError = new Error("cancelled by parent");
          const failures = {
            Left: new Error("left failed"),
            Right: new Error("right failed"),
          };
          const cancellation = new Error("child observed cancellation");
          const reasons = new Map<unknown, Reason>([
            [parentError, "ParentCancelled"],
            [failures.Left, "LeftFailed"],
            [failures.Right, "RightFailed"],
            [cancellation, "ChildCancelled"],
          ]);
          let cause: unknown;
          let returned: Reason | null = null;
          function decide(error: unknown, except?: Child) {
            if (cause === undefined || overwrite) cause = error;
            for (const child of children)
              if (child !== except && stages[child] === "Running")
                controllers[child].abort(cancellation);
          }
          parent.signal.addEventListener(
            "abort",
            () => decide(parent.signal.reason),
            { once: true },
          );
          const tasks = children.map(async (child) => {
            try {
              await work[child].promise;
            } catch (error) {
              const reason = reasons.get(error);
              assert.ok(reason);
              reported[child] = reason;
              if (error !== cancellation) decide(error, child);
              throw error;
            } finally {
              stages[child] = "Cleaning";
              await cleanup[child].promise;
              stages[child] = "Exited";
            }
          });
          const done = Promise.allSettled(tasks)
            .then(() => {
              if (cause !== undefined) throw cause;
            })
            .catch((error: unknown) => {
              const reason = reasons.get(error);
              assert.ok(reason);
              returned = reason;
            });
          const commands: Command[] = [
            ...scenario.base.commands,
            scenario.next,
            { kind: "acceptCancel", child: "Right" },
            { kind: "exitCleanup", child: "Left" },
            { kind: "exitCleanup", child: "Right" },
          ];
          const adapter = {
            async apply(command) {
              if (command.kind === "cancelParent") parent.abort(parentError);
              else if (command.kind === "fail")
                work[command.child].reject(failures[command.child]);
              else if (command.kind === "acceptCancel") {
                const signal = controllers[command.child].signal;
                assert.ok(signal.aborted);
                work[command.child].reject(signal.reason);
              } else cleanup[command.child].open();
              await setImmediate();
            },
            async observe() {
              return {
                cause: reasons.get(cause) ?? null,
                returned,
                stages: { ...stages },
                reported: { ...reported },
                signalled: children.filter(
                  (child) => controllers[child].signal.aborted,
                ),
              };
            },
            project: ({ cause, returned, stages, reported, signalled }) => ({
              cause,
              returned,
              stages,
              reported,
              signalled,
            }),
          } satisfies ObservationAdapter<Command>;
          try {
            const result = await recordReplay(
              checker,
              { ...scenario.base, commands },
              adapter,
            );
            if (overwrite) {
              assert.equal(result.verdict, "mismatch");
              assert.equal(result.difference.step, 2);
              assert.equal(
                (result.difference.observed as { cause: Reason }).cause,
                scenario.overwritten,
              );
              for (const command of commands.slice(result.difference.step))
                await adapter.apply(command);
              assert.equal(
                (await adapter.observe()).returned,
                scenario.overwritten,
              );
            } else {
              assert.equal(result.verdict, "matched");
              assert.equal(
                (
                  result.recording.observations.at(-2) as {
                    returned: Reason | null;
                  }
                ).returned,
                null,
              );
              assert.deepEqual(result.recording.observations.at(-1), {
                cause: scenario.expected,
                returned: scenario.expected,
                stages: { Left: "Exited", Right: "Exited" },
                reported: { Left: "LeftFailed", Right: "ChildCancelled" },
                signalled:
                  scenario.name === "cancel first"
                    ? ["Left", "Right"]
                    : ["Right"],
              });
            }
          } finally {
            for (const child of children) {
              work[child].open();
              cleanup[child].open();
            }
            await done;
          }
        },
      );
    }
  }
});
