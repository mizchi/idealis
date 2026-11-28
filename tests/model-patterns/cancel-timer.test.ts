import assert from "node:assert/strict";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Generation = "First" | "Second";
type Command =
  | { kind: "arm" | "settle"; generation: Generation }
  | { kind: "cancel" | "elapse" };
type Fault = "retain-handle" | "publish-obsolete";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/node-event-loop/cancel-timer.ideal";

function openTimer(t: TestContext, fault?: Fault) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let pending: {
    generation: Generation;
    handle: ReturnType<typeof setTimeout>;
  } | null = null;
  let current: Generation | null = null;
  let published: Generation | null = null;
  const used = new Set<Generation>();
  const running = new Map<
    Generation,
    { release: () => void; finished: Promise<void> }
  >();
  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "arm": {
          const { generation } = command;
          if (pending) clearTimeout(pending.handle);
          used.add(generation);
          current = generation;
          const handle = setTimeout(() => {
            if (pending?.generation === generation) pending = null;
            let release!: () => void;
            const gate = new Promise<void>((resolve) => {
              release = resolve;
            });
            const finished = gate.then(() => {
              if (fault === "publish-obsolete" || current === generation) {
                published = generation;
              }
              running.delete(generation);
            });
            running.set(generation, { release, finished });
          }, 1);
          pending = { generation, handle };
          break;
        }
        case "cancel":
          if (pending && fault !== "retain-handle")
            clearTimeout(pending.handle);
          pending = null;
          current = null;
          break;
        case "elapse":
          t.mock.timers.tick(1);
          break;
        case "settle": {
          const callback = running.get(command.generation);
          assert.ok(callback, "callback has not started");
          callback.release();
          await callback.finished;
          break;
        }
      }
    },
    async observe() {
      return {
        current,
        pending: pending?.generation ?? null,
        used: [...used].sort(),
        running: [...running.keys()].sort(),
        published,
      };
    },
    project: (observation: Readonly<Record<string, unknown>>) => observation,
  };
}

test("a timer cancellation separates stopping a reservation from fencing its completion", async (t) => {
  for (const [name, tail, fault, difference, expected, observed] of [
    [
      "FirstPending",
      [{ kind: "cancel" }, { kind: "elapse" }],
      "retain-handle",
      "running",
      [],
      ["First"],
    ],
    [
      "FirstRunning",
      [{ kind: "cancel" }, { kind: "settle", generation: "First" }],
      "publish-obsolete",
      "published",
      null,
      "First",
    ],
    [
      "NewPublishedWithOldRunning",
      [{ kind: "settle", generation: "First" }],
      "publish-obsolete",
      "published",
      "Second",
      "First",
    ],
  ] as const) {
    const prefix = await reachableScenario<Command>(checker, model, name, {
      checker: "rprob",
    });
    const scenario = { ...prefix, commands: [...prefix.commands, ...tail] };
    await t.test(name, async (t) => {
      const result = await recordReplay(checker, scenario, openTimer(t));
      assert.equal(result.verdict, "matched");
      assert.deepEqual(
        (result.recording.observations.at(-1) as Record<string, unknown>)[
          difference
        ],
        expected,
      );
    });
    await t.test(`${name} / ${fault}`, async (t) => {
      const result = await recordReplay(checker, scenario, openTimer(t, fault));
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, scenario.commands.length);
      assert.deepEqual(
        (result.difference.expected as Record<string, unknown>)[difference],
        expected,
      );
      assert.deepEqual(
        (result.difference.observed as Record<string, unknown>)[difference],
        observed,
      );
    });
  }
});
