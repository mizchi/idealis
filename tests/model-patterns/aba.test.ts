import assert from "node:assert/strict";
import { once } from "node:events";
import test from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Slot = "A" | "B" | "C";
type Command = { kind: "load" | "cas" } | { kind: "write"; destination: Slot };

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/aba/register.model";
const slots = ["A", "B", "C"];

test(
  "one atomic word distinguishes a fresh CAS from ABA until its tag wraps",
  { timeout: 30_000 },
  async () => {
    const stale = await reachableScenario<Command>(
      checker,
      model,
      "ReturnedToStaleValue",
      { checker: "rprob" },
    );
    const fresh = await reachableScenario<Command>(
      checker,
      model,
      "FreshRead",
      {
        checker: "rprob",
      },
    );
    assert.deepEqual(stale.commands, [
      { kind: "load" },
      { kind: "write", destination: "B" },
      { kind: "write", destination: "A" },
    ]);

    for (const [base, modulus] of [
      [fresh, 4],
      [stale, 4],
      [stale, 1],
      [stale, 2],
    ] as const) {
      const memory = new Int32Array(
        new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT),
      );
      const worker = () =>
        new Worker(
          `
      const { parentPort, workerData } = require("node:worker_threads");
      const { memory, modulus, slots } = workerData;
      const replace = (before, slot) => ((Math.floor(before / 4) + 1) % modulus) * 4 + slot;
      let saved;
      parentPort.on("message", command => {
        switch (command.kind) {
          case "load":
            saved = Atomics.load(memory, 0);
            parentPort.postMessage(saved);
            break;
          case "write":
            Atomics.store(memory, 0, replace(Atomics.load(memory, 0), slots.indexOf(command.destination)));
            parentPort.postMessage(null);
            break;
          case "cas":
            parentPort.postMessage(Atomics.compareExchange(memory, 0, saved, replace(saved, 2)));
            break;
        }
      });
    `,
          { eval: true, workerData: { memory, modulus, slots } },
        );
      const reader = worker();
      const writer = worker();
      let saved: number | null = null;
      let returned: number | null = null;
      try {
        const result = await recordReplay(
          checker,
          {
            ...base,
            commands: [...base.commands, { kind: "cas" }],
          },
          {
            async apply(command) {
              const target = command.kind === "write" ? writer : reader;
              const done = once(target, "message");
              target.postMessage(command);
              const [value] = await done;
              if (command.kind === "load") saved = value;
              if (command.kind === "cas") returned = value;
            },
            async observe() {
              return {
                slot: slots[Atomics.load(memory, 0) % 4],
                result:
                  returned === null
                    ? "Pending"
                    : returned === saved
                      ? "Applied"
                      : "Rejected",
              };
            },
            project(state) {
              return {
                slot: (state.current as { slot: Slot }).slot,
                result: state.result,
              };
            },
          },
        );
        if (modulus === 4) {
          assert.equal(result.verdict, "matched");
          assert.equal(saved, 0);
          assert.equal(returned, base === fresh ? 0 : 8);
          assert.equal(Atomics.load(memory, 0), base === fresh ? 6 : 8);
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.deepEqual(result.difference, {
            step: 4,
            command: { kind: "cas" },
            expected: { slot: "A", result: "Rejected" },
            observed: { slot: "C", result: "Applied" },
          });
          assert.equal(returned, saved);
        }
      } finally {
        await Promise.all([reader.terminate(), writer.terminate()]);
      }
    }
  },
);
