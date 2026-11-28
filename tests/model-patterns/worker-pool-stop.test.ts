import assert from "node:assert/strict";
import { once } from "node:events";
import test from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Job = "Active" | "Waiting";
type JobState = "Running" | "Queued" | "Reserved" | "Done" | "Cancelled";
type Command =
  | { readonly kind: "finish"; readonly job: Job }
  | { readonly kind: "reserve" | "dispatch" | "stop" | "join" };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/cancellation/worker-pool-stop.model";

test(
  "a worker pool cancels queued work and invalidates a scheduled dispatch during shutdown",
  { timeout: 30_000 },
  async (t) => {
    const waiting = await reachableScenario<Command>(
      checker,
      model,
      "SlotOccupiedWithWorkWaiting",
    );
    const reserved = await reachableScenario<Command>(
      checker,
      model,
      "PendingDispatchHasReservedSlot",
    );
    assert.deepEqual(waiting.commands, []);
    assert.deepEqual(reserved.commands, [
      { kind: "finish", job: "Active" },
      { kind: "reserve" },
    ]);
    const cases = [
      {
        name: "queued",
        base: waiting,
        faults: ["forget-queued"],
        tail: [
          { kind: "stop" },
          { kind: "join" },
          { kind: "finish", job: "Active" },
          { kind: "join" },
        ],
      },
      {
        name: "reserved",
        base: reserved,
        faults: ["dispatch-stale"],
        tail: [{ kind: "stop" }, { kind: "dispatch" }, { kind: "join" }],
      },
      {
        name: "dispatch while open",
        base: reserved,
        faults: [],
        tail: [
          { kind: "dispatch" },
          { kind: "finish", job: "Waiting" },
          { kind: "stop" },
          { kind: "join" },
        ],
      },
    ] as const;
    for (const scenario of cases) {
      for (const fault of ["none", ...scenario.faults] as const) {
        await t.test(`${scenario.name} / ${fault}`, async (t) => {
          const worker = new Worker(
            `
          const { parentPort } = require("node:worker_threads");
          let running = null;
          parentPort.on("message", (message) => {
            if (message.kind === "start" && running === null) {
              running = message.job;
              parentPort.postMessage({ kind: "started", job: running });
            } else if (message.kind === "finish" && running === message.job) {
              running = null;
              parentPort.postMessage({ kind: "finished", job: message.job });
            } else throw new Error("inconsistent worker command");
          });
        `,
            { eval: true },
          );
          t.after(() => worker.terminate());
          async function exchange(kind: "start" | "finish", job: Job) {
            const replied = once(worker, "message");
            worker.postMessage({ kind, job });
            const [message] = await replied;
            assert.deepEqual(message, {
              kind: kind === "start" ? "started" : "finished",
              job,
            });
          }
          await exchange("start", "Active");
          const started = new Set<Job>(["Active"]);
          const jobs: Record<Job, JobState> = {
            Active: "Running",
            Waiting: "Queued",
          };
          let phase: "Open" | "Stopping" | "Halted" = "Open";
          let scheduled: (() => Promise<void>) | undefined;
          const commands: Command[] = [
            ...scenario.base.commands,
            ...scenario.tail,
          ];
          const result = await recordReplay<Command>(
            checker,
            { ...scenario.base, commands },
            {
              async apply(command) {
                switch (command.kind) {
                  case "finish":
                    await exchange("finish", command.job);
                    jobs[command.job] = "Done";
                    break;
                  case "reserve":
                    jobs.Waiting = "Reserved";
                    scheduled = async () => {
                      if (
                        fault !== "dispatch-stale" &&
                        (phase !== "Open" || jobs.Waiting !== "Reserved")
                      )
                        return;
                      await exchange("start", "Waiting");
                      started.add("Waiting");
                      jobs.Waiting = "Running";
                    };
                    break;
                  case "dispatch": {
                    assert.ok(scheduled);
                    const callback = scheduled;
                    scheduled = undefined;
                    await callback();
                    break;
                  }
                  case "stop":
                    phase = "Stopping";
                    if (
                      jobs.Waiting === "Reserved" ||
                      (jobs.Waiting === "Queued" && fault !== "forget-queued")
                    )
                      jobs.Waiting = "Cancelled";
                    break;
                  case "join":
                    if (Object.values(jobs).includes("Running")) break;
                    await worker.terminate();
                    phase = "Halted";
                    break;
                }
              },
              async observe() {
                return {
                  phase,
                  jobs: { ...jobs },
                  started: [...started].sort(),
                  scheduled: scheduled !== undefined,
                };
              },
              project: ({ phase, jobs, started, scheduled }) => ({
                phase,
                jobs,
                started,
                scheduled,
              }),
            },
          );
          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(result.recording.observations.at(-1), {
              phase: "Halted",
              jobs: {
                Active: "Done",
                Waiting:
                  scenario.name === "dispatch while open"
                    ? "Done"
                    : "Cancelled",
              },
              started:
                scenario.name === "dispatch while open"
                  ? ["Active", "Waiting"]
                  : ["Active"],
              scheduled: false,
            });
          } else {
            assert.equal(result.verdict, "mismatch");
            assert.equal(
              result.difference.step,
              fault === "forget-queued" ? 1 : 4,
            );
            assert.deepEqual(
              result.difference.observed,
              fault === "forget-queued"
                ? {
                    phase: "Stopping",
                    jobs: { Active: "Running", Waiting: "Queued" },
                    started: ["Active"],
                    scheduled: false,
                  }
                : {
                    phase: "Stopping",
                    jobs: { Active: "Done", Waiting: "Running" },
                    started: ["Active", "Waiting"],
                    scheduled: false,
                  },
            );
          }
        });
      }
    }
  },
);
