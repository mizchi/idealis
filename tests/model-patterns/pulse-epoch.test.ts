import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = {
  kind: "check" | "park" | "publish" | "clear" | "notify" | "consume";
};
type Reader = {
  reader: "Idle" | "Checked" | "Parked" | "Ready" | "Finished";
  baseline: number | null;
  observed: number | null;
};

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/wait-notify/pulse-epoch.ideal";

function openPulse(t: TestContext, levelOnly = false) {
  const shared = new Int32Array(
    new SharedArrayBuffer(2 * Int32Array.BYTES_PER_ELEMENT),
  );
  const index = levelOnly ? 1 : 0;
  const worker = new Worker(
    `
    const { parentPort, workerData: { shared, index } } = require("node:worker_threads");
    const generation = () => Atomics.load(shared, 0);
    let reader = "Idle";
    let baseline = null;
    let observed = null;
    let waking;
    parentPort.on("message", async (kind) => {
      switch (kind) {
        case "check":
          baseline = generation();
          reader = "Checked";
          break;
        case "park": {
          const wait = Atomics.waitAsync(shared, index, index === 0 ? baseline : 0);
          if (wait.async) {
            reader = "Parked";
            waking = wait.value.then((result) => {
              if (result !== "ok") throw new Error(result);
              reader = generation() === baseline ? "Checked" : "Ready";
            });
          } else {
            if (wait.value !== "not-equal") throw new Error(wait.value);
            reader = "Ready";
          }
          break;
        }
        case "afterNotify":
          await waking;
          waking = undefined;
          break;
        case "consume":
          observed = generation();
          reader = "Finished";
          break;
      }
      parentPort.postMessage({ reader, baseline, observed });
    });
    `,
    { eval: true, workerData: { shared, index } },
  );
  t.after(async () => {
    await worker.terminate();
  });
  let reader: Reader = { reader: "Idle", baseline: null, observed: null };
  let signaled = 0;
  const wakes: number[] = [];
  async function send(kind: Command["kind"] | "afterNotify") {
    const response = once(worker, "message", {
      signal: AbortSignal.timeout(5_000),
    });
    worker.postMessage(kind);
    [reader] = (await response) as [Reader];
  }
  return {
    wakes,
    async apply({ kind }: Command) {
      switch (kind) {
        case "publish":
          Atomics.add(shared, 0, 1);
          Atomics.store(shared, 1, 1);
          break;
        case "clear":
          Atomics.store(shared, 1, 0);
          break;
        case "notify": {
          signaled = Atomics.load(shared, 0);
          const count = Atomics.notify(shared, index, 1);
          wakes.push(count);
          if (count > 0) await send("afterNotify");
          break;
        }
        default:
          await send(kind);
      }
    },
    async observe() {
      return {
        ...reader,
        generation: Atomics.load(shared, 0),
        level: Atomics.load(shared, 1) !== 0,
        signaled,
      };
    },
    project: (observation: Readonly<Record<string, unknown>>) => observation,
  };
}

test(
  "a pulse that returned to its original level still changes the wait epoch",
  { timeout: 30_000 },
  async (t) => {
    const early = await reachableScenario<Command>(
      checker,
      model,
      "ClearedAndNotifiedBeforePark",
      { checker: "rprob" },
    );
    const parked = await reachableScenario<Command>(
      checker,
      model,
      "ParkedBeforePulse",
      { checker: "rprob" },
    );
    const latePark = {
      ...early,
      commands: [...early.commands, { kind: "park" }, { kind: "consume" }],
    } satisfies typeof early;
    await t.test(
      "registration after the pulse reads a different epoch",
      async (t) => {
        const driver = openPulse(t);
        const result = await recordReplay(checker, latePark, driver);
        assert.equal(result.verdict, "matched");
        assert.deepEqual(driver.wakes, [0]);
        assert.deepEqual(result.recording.observations.at(-1), {
          reader: "Finished",
          generation: 1,
          signaled: 1,
          baseline: 0,
          observed: 1,
          level: false,
        });
      },
    );
    await t.test(
      "registration before the pulse is woken after the level cleared",
      async (t) => {
        const driver = openPulse(t);
        const result = await recordReplay(
          checker,
          {
            ...parked,
            commands: [
              ...parked.commands,
              { kind: "publish" },
              { kind: "clear" },
              { kind: "notify" },
              { kind: "consume" },
            ],
          },
          driver,
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(driver.wakes, [1]);
      },
    );
    await t.test(
      "waiting on the level can miss both the pulse and the notification",
      async (t) => {
        const driver = openPulse(t, true);
        const result = await recordReplay(checker, latePark, driver);
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, early.commands.length + 1);
        assert.deepEqual(result.difference.command, { kind: "park" });
        assert.deepEqual(driver.wakes, [0]);
        assert.deepEqual(result.difference.observed, {
          ...(result.difference.expected as Record<string, unknown>),
          reader: "Parked",
        });
      },
    );
  },
);
