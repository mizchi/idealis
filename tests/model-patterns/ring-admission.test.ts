import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = {
  kind: "inspect" | "retire" | "tryReuse" | "register" | "release";
};
type Fault = "none" | "ignore-retired" | "ignore-generation";
type Generation = "Old" | "Next";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/worker-deque/ring-admission.ideal";
const next = 4,
  open = 2,
  held = 1;
const generation = (word: number): Generation =>
  (word & next) === 0 ? "Old" : "Next";

function openSlot(t: TestContext, fault: Fault) {
  const state = new Int32Array(
    new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT),
  );
  Atomics.store(state, 0, open);
  const reader = new Worker(
    `
    const { parentPort, workerData } = require("node:worker_threads");
    const { state, next, open, held, fault } = workerData;
    let seen = null;
    parentPort.on("message", (kind) => {
      if (kind === "inspect") {
        const word = Atomics.load(state, 0);
        seen = word & open ? word & next : null;
        parentPort.postMessage(seen);
        return;
      }
      if (kind === "release") {
        Atomics.and(state, 0, ~held);
      } else if (kind === "register" && seen !== null) {
        while (true) {
          const word = Atomics.load(state, 0);
          if (word & held) break;
          if (!(word & open) && fault !== "ignore-retired") break;
          if ((word & next) !== seen && fault !== "ignore-generation") break;
          if (Atomics.compareExchange(state, 0, word, word | held) === word) break;
        }
      }
      parentPort.postMessage(null);
    });
  `,
    { eval: true, workerData: { state, next, open, held, fault } },
  );
  t.after(() => reader.terminate());
  let seen: Generation | null = null;
  return {
    async apply({ kind }: Command) {
      if (kind === "retire") Atomics.and(state, 0, ~open);
      else if (kind === "tryReuse") {
        // An old slot may be recycled only after it is closed and its pin is gone.
        Atomics.compareExchange(state, 0, 0, next | open);
      } else {
        const received = once(reader, "message");
        reader.postMessage(kind);
        const [word] = await received;
        if (kind === "inspect") {
          assert.ok(word === null || word === 0 || word === next);
          seen = word === null ? null : generation(word);
        }
      }
    },
    async observe() {
      const word = Atomics.load(state, 0);
      const current = generation(word);
      return {
        current,
        accepting: (word & open) !== 0,
        seen,
        pin: word & held ? current : null,
      };
    },
  };
}

test(
  "ring pins are admitted against the open generation seen by the reader",
  { timeout: 30_000 },
  async (t) => {
    const retired = await reachableScenario<Command>(
      checker,
      model,
      "ObservedBeforeRetirement",
    );
    const reused = await reachableScenario<Command>(
      checker,
      model,
      "OldViewAfterReuse",
    );
    const pinned = await reachableScenario<Command>(
      checker,
      model,
      "PinSurvivesRetirement",
    );
    assert.equal(retired.commands.length, 2);
    assert.equal(reused.commands.length, 3);
    assert.equal(pinned.commands.length, 3);
    const cases = [
      [retired, "none"],
      [retired, "ignore-retired"],
      [reused, "none"],
      [reused, "ignore-generation"],
      [pinned, "none"],
    ] as const;
    for (const [base, fault] of cases) {
      await t.test(
        `${base === pinned ? "existing pin" : base === retired ? "retired" : "reused"} / ${fault}`,
        async (t) => {
          const slot = openSlot(t, fault);
          const commands: Command[] = [...base.commands];
          if (base === pinned)
            commands.push(
              { kind: "tryReuse" },
              { kind: "release" },
              { kind: "tryReuse" },
            );
          else commands.push({ kind: "register" });
          if (base === retired) commands.push({ kind: "tryReuse" });
          if (base === reused)
            commands.push({ kind: "inspect" }, { kind: "register" });
          const result = await recordReplay(
            checker,
            { ...base, commands },
            {
              ...slot,
              project: ({ current, accepting, seen, pin }) => ({
                current,
                accepting,
                seen,
                pin,
              }),
            },
          );
          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            const checkpoint =
              result.recording.observations[base.commands.length + 1];
            assert.deepEqual(checkpoint, {
              current: base === reused ? "Next" : "Old",
              accepting: base === reused,
              seen: "Old",
              pin: base === pinned ? "Old" : null,
            });
            assert.deepEqual(await slot.observe(), {
              current: "Next",
              accepting: true,
              seen: base === reused ? "Next" : "Old",
              pin: base === reused ? "Next" : null,
            });
          } else {
            const current = fault === "ignore-retired" ? "Old" : "Next";
            assert.equal(result.verdict, "mismatch");
            assert.equal(result.difference.step, base.commands.length + 1);
            assert.deepEqual(result.difference.observed, {
              current,
              accepting: fault !== "ignore-retired",
              seen: "Old",
              pin: current,
            });
          }
        },
      );
    }
  },
);
