import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import test from "node:test";
import { Worker } from "node:worker_threads";
import {
  type ObservationAdapter,
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/wait-notify/shared-queue.model";
type Waiter = "Left" | "Right";
type Command =
  | { kind: "park" | "publish" | "resume"; worker: Waiter }
  | { kind: "notify" };

test(
  "a FIFO notification can miss the worker whose condition is ready",
  { timeout: 30_000 },
  async () => {
    const scenario = await reachableScenario<Command>(
      checker,
      model,
      "LeftBeforeRight",
    );
    assert.deepEqual(scenario.commands, [
      { kind: "park", worker: "Left" },
      { kind: "park", worker: "Right" },
    ]);
    for (const single of [false, true]) {
      const shared = new Int32Array(
        new SharedArrayBuffer(3 * Int32Array.BYTES_PER_ELEMENT),
      );
      const events = new EventEmitter();
      const woken = new Set<Waiter>();
      const finished = new Set<Waiter>();
      const worker = (name: Waiter, index: number) => {
        const bridge = new Worker(
          `
        const { parentPort, workerData } = require("node:worker_threads");
        const { shared, index } = workerData;
        parentPort.on("message", async (kind) => {
          if (kind === "park") {
            const wait = Atomics.waitAsync(shared, 0, 0);
            if (!wait.async) throw new Error("did not register a wait");
            parentPort.postMessage("parked");
            const answer = await wait.value;
            if (answer !== "ok") throw new Error(answer);
            parentPort.postMessage("woken");
          } else {
            parentPort.postMessage(Atomics.load(shared, index) === 1 ? "finished" : "not-ready");
          }
        });
      `,
          { eval: true, workerData: { shared, index } },
        );
        bridge.on("message", (message) => {
          if (message === "woken") {
            woken.add(name);
            events.emit("wake");
          }
        });
        bridge.on("error", (error) => events.emit("worker-error", error));
        return bridge;
      };
      const workers = { Left: worker("Left", 1), Right: worker("Right", 2) };
      const runtimeErrors: unknown[] = [];
      events.on("worker-error", (error) => runtimeErrors.push(error));
      const wakeCounts: number[] = [];
      async function awaitWakes(total: number) {
        const deadline = AbortSignal.timeout(5_000);
        while (woken.size < total) {
          await once(events, "wake", { signal: deadline });
        }
      }
      try {
        const adapter: ObservationAdapter<Command> = {
          async apply(command) {
            if (command.kind === "publish") {
              Atomics.store(shared, command.worker === "Left" ? 1 : 2, 1);
            } else if (command.kind === "notify") {
              const wakeCount = Atomics.notify(shared, 0, single ? 1 : 2);
              wakeCounts.push(wakeCount);
              await awaitWakes(wakeCount);
            } else {
              const bridge = workers[command.worker];
              const message = once(bridge, "message", {
                signal: AbortSignal.timeout(5_000),
              });
              bridge.postMessage(command.kind);
              const [answer] = await message;
              if (command.kind === "park") {
                assert.equal(answer, "parked");
              } else {
                assert.ok(woken.delete(command.worker));
                if (answer === "finished") finished.add(command.worker);
                else assert.equal(answer, "not-ready");
              }
            }
          },
          async observe() {
            assert.deepEqual(runtimeErrors, []);
            return { woken: [...woken].sort(), finished: [...finished].sort() };
          },
          project: ({ woken, finished }) => ({
            woken: [...(woken as Waiter[])].sort(),
            finished: [...(finished as Waiter[])].sort(),
          }),
        };
        const commands: Command[] = [
          ...scenario.commands,
          { kind: "publish", worker: "Right" },
          { kind: "notify" },
          { kind: "resume", worker: "Left" },
          { kind: "resume", worker: "Right" },
        ];
        const result = await recordReplay(
          checker,
          { ...scenario, commands },
          adapter,
        );
        assert.deepEqual(wakeCounts, [single ? 1 : 2]);
        if (single) {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, 4);
          assert.deepEqual(result.difference.observed, {
            woken: ["Left"],
            finished: [],
          });
          assert.equal(Atomics.notify(shared, 0, 1), 1);
          await awaitWakes(2);
          assert.ok(woken.has("Right"));
        } else {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations.at(-1), {
            woken: [],
            finished: ["Right"],
          });
        }
      } finally {
        await Promise.all(
          Object.values(workers).map((bridge) => bridge.terminate()),
        );
      }
    }
  },
);
