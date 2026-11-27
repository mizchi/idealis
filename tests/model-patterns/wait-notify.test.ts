import assert from "node:assert/strict";
import { once } from "node:events";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
  repeatedTemporalScenario,
} from "../../examples/model-patterns/replay.js";

type Command = {
  kind: "check" | "park" | "publish" | "notify" | "ping" | "consume";
};
type Stage = "Idle" | "Checked" | "Parked" | "Ready" | "Finished";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/wait-notify/wait.model";

test(
  "wait registration handles early signals and notifications with a false predicate",
  { timeout: 30_000 },
  async (t) => {
    const early = await reachableScenario<Command>(
      checker,
      model,
      "NotifiedBeforePark",
    );
    const parked = await reachableScenario<Command>(
      checker,
      model,
      "ParkedBeforePublication",
    );
    assert.equal(early.commands.length, 3);
    assert.equal(parked.commands.length, 2);
    const dir = await mkdtemp(join(tmpdir(), "idealis-lost-wakeup-"));
    t.after(() => rm(dir, { recursive: true, force: true }));
    const faultyModel = join(dir, "fault.model");
    const text = await readFile(model, "utf8");
    await writeFile(
      faultyModel,
      text.replace(
        "reader = if { ready } then Ready else Parked",
        "reader = Parked",
      ),
    );
    const stuck = await repeatedTemporalScenario<Command>(
      checker,
      faultyModel,
      "CompletesAfterNotify",
      0,
    );
    assert.deepEqual(stuck.commands, [...early.commands, { kind: "park" }]);
    await assert.rejects(
      repeatedTemporalScenario(checker, faultyModel, "CompletesAfterNotify", 1),
      /loop has no executable commands/,
    );

    for (const [base, suffix, fault, signalled] of [
      [early, ["park", "consume"], "none", [0]],
      [{ ...stuck, model }, [], "lost-wakeup", [0]],
      [parked, ["publish", "notify", "consume"], "none", [1]],
      [
        parked,
        ["ping", "check", "park", "publish", "notify", "consume"],
        "none",
        [1, 1],
      ],
      [parked, ["ping"], "unconditional-wake", [1]],
    ] as const) {
      const shared = new Int32Array(
        new SharedArrayBuffer(2 * Int32Array.BYTES_PER_ELEMENT),
      );
      const index = fault === "lost-wakeup" ? 1 : 0;
      const worker = new Worker(
        `
      const { parentPort, workerData } = require("node:worker_threads");
      const { shared, index, fault } = workerData;
      const ready = () => Atomics.load(shared, 0) === 1;
      let stage = "Idle";
      let wake;
      parentPort.on("message", async (kind) => {
        switch (kind) {
          case "check":
            stage = ready() ? "Ready" : "Checked";
            break;
          case "park": {
            const wait = Atomics.waitAsync(shared, index, 0);
            if (wait.async) {
              stage = "Parked";
              wake = wait.value.then((reply) => {
                if (reply !== "ok") throw new Error(reply);
                stage = ready() || fault === "unconditional-wake" ? "Ready" : "Idle";
              });
            } else {
              if (wait.value !== "not-equal") throw new Error(wait.value);
              stage = "Ready";
            }
            break;
          }
          case "afterNotify":
            await wake;
            wake = undefined;
            break;
          case "consume":
            if (!ready()) throw new Error("condition was false");
            stage = "Finished";
            break;
        }
        parentPort.postMessage(stage);
      });
    `,
        { eval: true, workerData: { shared, index, fault } },
      );
      const wakeCounts: number[] = [];
      let stage: Stage = "Idle";
      async function send(kind: string) {
        const reply = once(worker, "message", {
          signal: AbortSignal.timeout(5_000),
        });
        worker.postMessage(kind);
        [stage] = (await reply) as [Stage];
      }
      try {
        const result = await recordReplay<Command>(
          checker,
          {
            ...base,
            commands: [...base.commands, ...suffix.map((kind) => ({ kind }))],
          },
          {
            async apply({ kind }) {
              if (kind === "publish") {
                Atomics.store(shared, 0, 1);
              } else if (kind === "notify" || kind === "ping") {
                const count = Atomics.notify(shared, index, 1);
                wakeCounts.push(count);
                if (count > 0) await send("afterNotify");
              } else {
                await send(kind);
              }
            },
            async observe() {
              return { ready: Atomics.load(shared, 0) === 1, reader: stage };
            },
            project: ({ ready, reader }) => ({ ready, reader }),
          },
        );
        assert.deepEqual(wakeCounts, signalled);
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations.at(-1), {
            ready: true,
            reader: "Finished",
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(
            result.difference.step,
            base.commands.length + suffix.length,
          );
          assert.deepEqual(
            result.difference.observed,
            fault === "lost-wakeup"
              ? { ready: true, reader: "Parked" }
              : { ready: false, reader: "Ready" },
          );
        }
      } finally {
        await worker.terminate();
      }
    }
  },
);
