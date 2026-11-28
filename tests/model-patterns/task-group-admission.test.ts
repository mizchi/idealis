import assert from "node:assert/strict";
import test from "node:test";
import { setImmediate } from "node:timers/promises";
import {
  reachableScenario,
  recordReplay,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";
import { taskGate } from "./helpers/task-gate.js";

type Child = "Initial" | "Added";
type Command =
  | { readonly kind: "offer" | "close" }
  | { readonly kind: "finish"; readonly child: Child };
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/cancellation/task-group-admission.ideal";

test("closing a task group waits for registrations before the boundary and rejects later child starts", async (t) => {
  const before = await reachableScenario<Command>(
    checker,
    model,
    "AcceptedBeforeClosure",
  );
  const after = await reachableScenario<Command>(
    checker,
    model,
    "ClosedWhileInitialRuns",
  );
  assert.deepEqual(before.commands, [{ kind: "offer" }, { kind: "close" }]);
  assert.deepEqual(after.commands, [{ kind: "close" }]);
  const cases = [
    {
      name: "accepted before close",
      base: before,
      fault: "snapshot-early",
      tail: [
        { kind: "finish", child: "Initial" },
        { kind: "finish", child: "Added" },
      ],
      expected: "Accepted",
    },
    {
      name: "offered after close",
      base: after,
      fault: "allow-late",
      tail: [{ kind: "offer" }, { kind: "finish", child: "Initial" }],
      expected: "Rejected",
    },
  ] as const;
  for (const scenario of cases) {
    for (const fault of ["none", scenario.fault] as const) {
      await t.test(`${scenario.name} / ${fault}`, async () => {
        const gates = { Initial: taskGate(), Added: taskGate() };
        const tasks = new Map<Child, Promise<void>>();
        const started = new Set<Child>();
        const running = new Set<Child>();
        let accepting = true;
        let joined = false;
        let decision: "Accepted" | "Rejected" | null = null;
        let join: Promise<void> | undefined;
        function start(child: Child) {
          tasks.set(
            child,
            (async () => {
              started.add(child);
              running.add(child);
              await gates[child].promise;
              running.delete(child);
            })(),
          );
        }
        start("Initial");
        const initialTasks = [...tasks.values()];
        const adapter = {
          async apply(command) {
            if (command.kind === "offer") {
              if (accepting || fault === "allow-late") {
                decision = "Accepted";
                start("Added");
              } else decision = "Rejected";
            } else if (command.kind === "close") {
              accepting = false;
              join = Promise.all(
                fault === "snapshot-early" ? initialTasks : tasks.values(),
              ).then(() => {
                joined = true;
              });
            } else if (command.kind === "finish") {
              gates[command.child].open();
              await tasks.get(command.child);
            }
            await setImmediate();
          },
          async observe() {
            return {
              phase: joined ? "Joined" : accepting ? "Open" : "Closing",
              decision,
              running: [...running].sort(),
              started: [...started].sort(),
            };
          },
          project: ({ phase, decision, running, started }) => ({
            phase,
            decision,
            running,
            started,
          }),
        } satisfies ObservationAdapter<Command>;
        try {
          const commands: Command[] = [
            ...scenario.base.commands,
            ...scenario.tail,
          ];
          const result = await recordReplay(
            checker,
            { ...scenario.base, commands },
            adapter,
          );
          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(result.recording.observations.at(-1), {
              phase: "Joined",
              decision: scenario.expected,
              running: [],
              started:
                scenario.expected === "Accepted"
                  ? ["Added", "Initial"]
                  : ["Initial"],
            });
          } else {
            assert.equal(result.verdict, "mismatch");
            assert.equal(
              result.difference.step,
              fault === "snapshot-early" ? 3 : 2,
            );
            assert.deepEqual(
              result.difference.observed,
              fault === "snapshot-early"
                ? {
                    phase: "Joined",
                    decision: "Accepted",
                    running: ["Added"],
                    started: ["Added", "Initial"],
                  }
                : {
                    phase: "Closing",
                    decision: "Accepted",
                    running: ["Added", "Initial"],
                    started: ["Added", "Initial"],
                  },
            );
            if (fault === "allow-late") {
              await adapter.apply({ kind: "finish", child: "Initial" });
              assert.deepEqual(await adapter.observe(), {
                phase: "Joined",
                decision: "Accepted",
                running: ["Added"],
                started: ["Added", "Initial"],
              });
            }
          }
        } finally {
          gates.Initial.open();
          gates.Added.open();
          await join;
          await Promise.all(tasks.values());
        }
      });
    }
  }
});
