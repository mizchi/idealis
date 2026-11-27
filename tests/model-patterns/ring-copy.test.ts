import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command =
  | { kind: "pin" | "finish" | "tryReuse" }
  | { kind: "copyPart"; field: "Header" | "Data" };
type ReaderView = {
  phase: "Idle" | "Copying" | "Returned";
  header: number;
  data: number;
};

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/worker-deque/ring-copy.model";

function openSlot(t: TestContext, releaseAfterHeader: boolean) {
  const words = new Int32Array(
    new SharedArrayBuffer(3 * Int32Array.BYTES_PER_ELEMENT),
  );
  const states = ["Published", "Held", "Free", "Refilled"] as const;
  words.set([0, 11, 12]);
  function start(role: "producer" | "reader") {
    const worker = new Worker(
      `
      const { parentPort, workerData } = require("node:worker_threads");
      const { words, role, releaseAfterHeader } = workerData;
      const published = 0, held = 1, free = 2, refilled = 3, writing = 4;
      let phase = "Idle", header = 0, data = 0;
      parentPort.on("message", (command) => {
        if (role === "producer") {
          const reused = Atomics.compareExchange(words, 0, free, writing) === free;
          if (reused) {
            Atomics.store(words, 1, 21);
            Atomics.store(words, 2, 22);
            Atomics.store(words, 0, refilled);
          }
          parentPort.postMessage(reused);
          return;
        }
        switch (command.kind) {
          case "pin":
            if (Atomics.compareExchange(words, 0, published, held) !== published) throw new Error("slot is not published");
            phase = "Copying";
            break;
          case "copyPart":
            if (phase !== "Copying") throw new Error("copy outside reader lifetime");
            if (command.field === "Header") {
              header = Atomics.load(words, 1);
              if (releaseAfterHeader) Atomics.store(words, 0, free);
            } else {
              data = Atomics.load(words, 2);
            }
            break;
          case "finish":
            if (header === 0 || data === 0) throw new Error("copy incomplete");
            Atomics.compareExchange(words, 0, held, free);
            phase = "Returned";
            break;
        }
        parentPort.postMessage({ phase, header, data });
      });
    `,
      { eval: true, workerData: { words, role, releaseAfterHeader } },
    );
    t.after(() => worker.terminate());
    return worker;
  }
  const producer = start("producer");
  const reader = start("reader");
  let view: ReaderView = { phase: "Idle", header: 0, data: 0 };
  function generation(word: number, old: number, next: number) {
    if (word === old) return "Old";
    assert.equal(word, next);
    return "Next";
  }
  return {
    async apply(command: Command) {
      const worker = command.kind === "tryReuse" ? producer : reader;
      const response = once(worker, "message");
      worker.postMessage(command);
      const [message] = await response;
      if (command.kind !== "tryReuse") view = message;
    },
    async observe() {
      return {
        slot: states[Atomics.load(words, 0)],
        reader: view.phase,
        body: {
          Header: generation(Atomics.load(words, 1), 11, 21),
          Data: generation(Atomics.load(words, 2), 12, 22),
        },
        copy: {
          Header: view.header === 0 ? null : generation(view.header, 11, 21),
          Data: view.data === 0 ? null : generation(view.data, 12, 22),
        },
      };
    },
  };
}

test(
  "the ring slot stays held until the reader has copied both words",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "PausedAfterHeader",
    );
    assert.deepEqual(base.commands, [
      { kind: "pin" },
      { kind: "copyPart", field: "Header" },
    ]);
    for (const early of [false, true]) {
      await t.test(
        early ? "release after header" : "release after full copy",
        async (t) => {
          const slot = openSlot(t, early);
          const rest: readonly Command[] = [
            { kind: "tryReuse" },
            { kind: "copyPart", field: "Data" },
            { kind: "finish" },
            { kind: "tryReuse" },
          ];
          const result = await recordReplay(
            checker,
            { ...base, commands: [...base.commands, ...rest] },
            {
              ...slot,
              project: ({ slot, reader, body, copy }) => ({
                slot,
                reader,
                body,
                copy,
              }),
            },
          );
          if (early) {
            assert.equal(result.verdict, "mismatch");
            assert.equal(result.difference.step, 2);
            for (const command of rest.slice(0, 3)) await slot.apply(command);
          } else {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(result.recording.observations[3], {
              slot: "Held",
              reader: "Copying",
              body: { Header: "Old", Data: "Old" },
              copy: { Header: "Old", Data: null },
            });
          }
          assert.deepEqual(await slot.observe(), {
            slot: "Refilled",
            reader: "Returned",
            body: { Header: "Next", Data: "Next" },
            copy: { Header: "Old", Data: early ? "Next" : "Old" },
          });
        },
      );
    }
  },
);
