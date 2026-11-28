import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const participants = ["Fast", "Slow"] as const;
const rounds = ["R1", "R2", "Done"] as const;
type Participant = (typeof participants)[number];
type Command = { kind: "arrive" | "tryPass"; worker: Participant };

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/worker-barrier/reuse.ideal";

function sharedBarrier(t: TestContext, staleRelease: boolean) {
  const shared = new Int32Array(
    new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT),
  );
  const start = () => {
    const worker = new Worker(
      `
      const { parentPort, workerData: { shared, staleRelease } } = require("node:worker_threads");
      let round = 0;
      let waiting = false;
      parentPort.on("message", (kind) => {
        if (kind === "arrive") {
          // Low bit records the first arrival. The second publishes the
          // next generation and clears that bit in one atomic update.
          let word = Atomics.load(shared, 0);
          for (;;) {
            if (waiting || (word >> 1) !== round) throw new Error("unexpected arrival");
            const previous = Atomics.compareExchange(shared, 0, word, word + 1);
            if (previous === word) break;
            word = previous;
          }
          waiting = true;
        } else {
          if (!waiting) throw new Error("unexpected wait");
          const released = staleRelease
            ? Atomics.load(shared, 0) >= 2
            : Atomics.wait(shared, 0, 2 * round + 1, 0) === "not-equal";
          if (released) {
            round += 1;
            waiting = false;
          }
        }
        parentPort.postMessage({ round, waiting });
      });
      `,
      { eval: true, workerData: { shared, staleRelease } },
    );
    t.after(() => worker.terminate());
    return worker;
  };
  const workers = { Fast: start(), Slow: start() };
  const local = {
    Fast: { round: 0, waiting: false },
    Slow: { round: 0, waiting: false },
  };
  return {
    async apply(command: Command) {
      const worker = workers[command.worker];
      const replied = once(worker, "message");
      worker.postMessage(command.kind);
      const [status] = await replied;
      local[command.worker] = status;
    },
    async observe() {
      const word = Atomics.load(shared, 0);
      return {
        generation: rounds[word >> 1],
        count: word & 1,
        rounds: {
          Fast: rounds[local.Fast.round],
          Slow: rounds[local.Slow.round],
        },
        waiting: participants.filter((worker) => local[worker].waiting),
      };
    },
    project(state: Readonly<Record<string, unknown>>) {
      return {
        generation: state.generation,
        count: (state.arrived as readonly unknown[]).length,
        rounds: state.rounds,
        waiting: state.waiting,
      };
    },
  };
}

test(
  "a reusable worker barrier requires fresh arrivals after releasing the previous round",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "FastArrivesWhileSlowStillWaits",
    );
    assert.equal(base.commands.length, 4);
    const commands: Command[] = [
      ...base.commands,
      { kind: "tryPass", worker: "Fast" },
      { kind: "tryPass", worker: "Slow" },
      { kind: "arrive", worker: "Slow" },
      { kind: "tryPass", worker: "Fast" },
      { kind: "tryPass", worker: "Slow" },
    ];
    for (const staleRelease of [false, true]) {
      await t.test(
        staleRelease
          ? "remembering any release passes too early"
          : "waiting compares the active generation",
        async (t) => {
          const result = await recordReplay(
            checker,
            { ...base, commands },
            sharedBarrier(t, staleRelease),
          );
          const blocked = {
            generation: "R2",
            count: 1,
            rounds: { Fast: "R2", Slow: "R1" },
            waiting: ["Fast", "Slow"],
          };
          if (staleRelease) {
            assert.equal(result.verdict, "mismatch");
            assert.deepEqual(result.difference, {
              step: 5,
              command: { kind: "tryPass", worker: "Fast" },
              expected: blocked,
              observed: {
                ...blocked,
                rounds: { Fast: "Done", Slow: "R1" },
                waiting: ["Slow"],
              },
            });
          } else {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(result.recording.observations[5], blocked);
            assert.deepEqual(result.recording.observations.at(-1), {
              generation: "Done",
              count: 0,
              rounds: { Fast: "Done", Slow: "Done" },
              waiting: [],
            });
          }
        },
      );
    }
  },
);
