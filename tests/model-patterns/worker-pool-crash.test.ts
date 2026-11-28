import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";

type Job = "Active" | "Waiting";
type Generation = "Original" | "Replacement";
type Command = {
  readonly kind: "crash" | "replace" | "startWaiting" | "finish";
};

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/cancellation/worker-pool-crash.model";

function spawn(t: TestContext, generation: Generation) {
  const worker = new Worker(
    `
    const { parentPort, workerData } = require("node:worker_threads");
    let current = null;
    parentPort.on("message", (command) => {
      if (command.kind === "crash") throw new Error("worker failed before replying");
      if (command.kind === "start" && current === null) current = command.job;
      else if (command.kind === "finish" && current === command.job) current = null;
      else throw new Error("invalid worker command");
      parentPort.postMessage({ kind: command.kind, job: command.job, generation: workerData });
    });
  `,
    { eval: true, workerData: generation },
  );
  // Observe exit even after an error. events.once would reject before exit.
  const exit = new Promise<number>((resolve) => worker.once("exit", resolve));
  t.after(() => worker.terminate());
  return { worker, exit, generation };
}

async function exchange(
  peer: ReturnType<typeof spawn>,
  kind: "start" | "finish",
  job: Job,
) {
  const received = once(peer.worker, "message");
  peer.worker.postMessage({ kind, job });
  const [message] = await received;
  assert.deepEqual(message, { kind, job, generation: peer.generation });
}

test(
  "a genuine worker exit fails its job and frees the slot for a replacement",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "CrashWasObserved",
    );
    assert.deepEqual(base.commands, [{ kind: "crash" }]);
    const commands: Command[] = [
      ...base.commands,
      { kind: "replace" },
      { kind: "startWaiting" },
      { kind: "finish" },
    ];
    for (const fault of ["none", "report-success", "keep-slot"] as const) {
      await t.test(fault, async (t) => {
        let peer: ReturnType<typeof spawn> | undefined = spawn(t, "Original");
        await exchange(peer, "start", "Active");
        let slot: Job | null = "Active";
        const started = new Set<Job>(["Active"]);
        const jobs: Record<Job, "Running" | "Failed" | "Queued" | "Done"> = {
          Active: "Running",
          Waiting: "Queued",
        };
        const adapter = {
          async apply({ kind }) {
            switch (kind) {
              case "crash": {
                assert.ok(peer && slot === "Active");
                const failure = once(peer.worker, "error");
                peer.worker.postMessage({ kind: "crash" });
                const [error] = await failure;
                assert.ok(
                  error instanceof Error &&
                    error.message === "worker failed before replying",
                );
                assert.notEqual(await peer.exit, 0);
                peer = undefined;
                jobs.Active = fault === "report-success" ? "Done" : "Failed";
                if (fault !== "keep-slot") slot = null;
                break;
              }
              case "replace":
                assert.equal(peer, undefined);
                peer = spawn(t, "Replacement");
                await once(peer.worker, "online");
                break;
              case "startWaiting":
                if (peer && slot === null) {
                  await exchange(peer, "start", "Waiting");
                  slot = "Waiting";
                  jobs.Waiting = "Running";
                  started.add("Waiting");
                }
                break;
              case "finish":
                assert.ok(peer && slot === "Waiting");
                await exchange(peer, "finish", "Waiting");
                jobs.Waiting = "Done";
                slot = null;
                break;
            }
          },
          async observe() {
            return {
              live: peer?.generation ?? null,
              slot,
              jobs: { ...jobs },
              started: [...started].sort(),
            };
          },
          project: ({ live, slot, jobs, started }) => ({
            live,
            slot,
            jobs,
            started,
          }),
        } satisfies ObservationAdapter<Command>;
        const result = await recordReplay(
          checker,
          { ...base, commands },
          adapter,
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations.at(-1), {
            live: "Replacement",
            slot: null,
            jobs: { Active: "Failed", Waiting: "Done" },
            started: ["Active", "Waiting"],
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, 1);
          assert.deepEqual(result.difference.observed, {
            live: null,
            slot: fault === "keep-slot" ? "Active" : null,
            jobs: {
              Active: fault === "report-success" ? "Done" : "Failed",
              Waiting: "Queued",
            },
            started: ["Active"],
          });
          if (fault === "keep-slot") {
            await adapter.apply({ kind: "replace" });
            await adapter.apply({ kind: "startWaiting" });
            assert.deepEqual(await adapter.observe(), {
              live: "Replacement",
              slot: "Active",
              jobs: { Active: "Failed", Waiting: "Queued" },
              started: ["Active"],
            });
          }
        }
      });
    }
  },
);
