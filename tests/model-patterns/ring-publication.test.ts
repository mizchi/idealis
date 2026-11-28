import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Field = "Header" | "Data";
type Ticket = "Old" | "Next";
type Command =
  | { kind: "reserve" | "publish" | "read" }
  | { kind: "writePart"; field: Field };
type Timing = "after-both" | "on-reserve" | "after-header";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/worker-deque/ring-publication.model";

function openSlot(t: TestContext, timing: Timing) {
  const words = new Int32Array(
    new SharedArrayBuffer(4 * Int32Array.BYTES_PER_ELEMENT),
  );
  // Reserved flag, published ticket, header word, data word.
  words.set([0, 0, 11, 12]);
  function start(role: "producer" | "consumer") {
    const worker = new Worker(
      `
      const { parentPort, workerData } = require("node:worker_threads");
      const { words, role, timing } = workerData;
      parentPort.on("message", (command) => {
        if (role === "consumer") {
          const ticket = Atomics.load(words, 1);
          parentPort.postMessage(ticket === 0 ? null : [ticket, Atomics.load(words, 2), Atomics.load(words, 3)]);
          return;
        }
        switch (command.kind) {
          case "reserve":
            Atomics.store(words, 0, 1);
            if (timing === "on-reserve") Atomics.store(words, 1, 2);
            break;
          case "writePart":
            if (command.field === "Header") {
              Atomics.store(words, 2, 21);
              if (timing === "after-header") Atomics.store(words, 1, 2);
            } else {
              Atomics.store(words, 3, 22);
            }
            break;
          case "publish":
            Atomics.store(words, 1, 2);
            break;
        }
        parentPort.postMessage(null);
      });
    `,
      { eval: true, workerData: { words, role, timing } },
    );
    t.after(() => worker.terminate());
    return worker;
  }
  const producer = start("producer");
  const consumer = start("consumer");
  function decode(word: number, old: number, next: number): Ticket {
    if (word === old) return "Old";
    assert.equal(word, next);
    return "Next";
  }
  let copied: readonly [number, number, number] | null = null;
  return {
    async apply(command: Command) {
      const worker = command.kind === "read" ? consumer : producer;
      const reply = once(worker, "message");
      worker.postMessage(command);
      const [message] = await reply;
      if (command.kind === "read") copied = message;
    },
    async observe() {
      const visible = Atomics.load(words, 1);
      return {
        reserved: Atomics.load(words, 0) === 1,
        visible: visible === 0 ? null : decode(visible, 1, 2),
        body: {
          Header: decode(Atomics.load(words, 2), 11, 21),
          Data: decode(Atomics.load(words, 3), 12, 22),
        },
        seen: copied === null ? null : decode(copied[0], 1, 2),
        copy: {
          Header: copied === null ? null : decode(copied[1], 11, 21),
          Data: copied === null ? null : decode(copied[2], 12, 22),
        },
      };
    },
  };
}

test(
  "a reader sees both payload words for the published ticket",
  { timeout: 30_000 },
  async (t) => {
    const staged = await reachableScenario<Command>(
      checker,
      model,
      "HeaderStagedWithoutPublication",
    );
    const ready = await reachableScenario<Command>(
      checker,
      model,
      "PublishedUnread",
    );
    assert.deepEqual(staged.commands, [
      { kind: "reserve" },
      { kind: "writePart", field: "Header" },
    ]);
    assert.equal(ready.commands.length, 4);
    for (const timing of [
      "after-both",
      "on-reserve",
      "after-header",
    ] as const) {
      await t.test(timing, async (t) => {
        const slot = openSlot(t, timing);
        const base = timing === "after-both" ? ready : staged;
        const commands: Command[] = [...base.commands];
        if (timing === "after-both") {
          commands.splice(2, 0, { kind: "read" });
          commands.push({ kind: "read" });
        }
        const result = await recordReplay(
          checker,
          { ...base, commands },
          {
            ...slot,
            project: ({ reserved, visible, body, seen, copy }) => ({
              reserved,
              visible,
              body,
              seen,
              copy,
            }),
          },
        );
        if (timing === "after-both") {
          assert.equal(result.verdict, "matched");
          const stagedRead = result.recording.observations[3] as Awaited<
            ReturnType<typeof slot.observe>
          >;
          assert.equal(stagedRead.seen, null);
          assert.deepEqual(stagedRead.copy, { Header: null, Data: null });
          assert.deepEqual(result.recording.observations.at(-1), {
            reserved: true,
            visible: "Next",
            body: { Header: "Next", Data: "Next" },
            seen: "Next",
            copy: { Header: "Next", Data: "Next" },
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, timing === "on-reserve" ? 1 : 2);
          await slot.apply({ kind: "read" });
          const observation = await slot.observe();
          assert.equal(observation.seen, "Next");
          assert.deepEqual(observation.copy, {
            Header: timing === "on-reserve" ? "Old" : "Next",
            Data: "Old",
          });
        }
      });
    }
  },
);
