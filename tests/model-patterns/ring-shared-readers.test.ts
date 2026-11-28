import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const readers = ["First", "Second"] as const;
type Reader = (typeof readers)[number];
type Ticket = "Old" | "Next";
type Command =
  | { kind: "stop" | "reclaim" | "copy" | "finish"; reader: Reader }
  | { kind: "tryReuse" };
type Fault = "none" | "clear-all" | "double-debit";
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/worker-deque/ring-shared-readers.ideal";

async function openSlot(t: TestContext, fault: Fault) {
  // Reader bits, reference count, header and data.
  const words = new Int32Array(
    new SharedArrayBuffer(4 * Int32Array.BYTES_PER_ELEMENT),
  );
  words.set([3, 2, 11, 12]);
  const workers = new Map<Reader, Worker>();
  for (const [index, reader] of readers.entries()) {
    const worker = new Worker(
      `
      const { parentPort, workerData } = require("node:worker_threads");
      const { words, bit } = workerData;
      const header = Atomics.load(words, 2);
      parentPort.on("message", (kind) => {
        if (kind === "copy") parentPort.postMessage([header, Atomics.load(words, 3)]);
        else if (kind === "finish") {
          if (Atomics.and(words, 0, ~bit) & bit) Atomics.sub(words, 1, 1);
          parentPort.postMessage(null);
        }
      });
      parentPort.postMessage(header);
    `,
      { eval: true, workerData: { words, bit: 1 << index } },
    );
    t.after(() => worker.terminate());
    workers.set(reader, worker);
    assert.deepEqual(await once(worker, "message"), [11]);
  }
  const stopped = new Set<Reader>();
  const finished = new Set<Reader>();
  const copies: Record<Reader, Ticket | null> = { First: null, Second: null };
  return {
    async apply(command: Command) {
      if (command.kind === "tryReuse") {
        if (Atomics.load(words, 1) === 0) {
          Atomics.store(words, 2, 21);
          Atomics.store(words, 3, 22);
        }
        return;
      }
      const { reader } = command;
      const worker = workers.get(reader);
      assert.ok(worker);
      if (command.kind === "stop") {
        const exit = once(worker, "exit");
        const code = await worker.terminate();
        assert.notEqual(code, 0);
        assert.deepEqual(await exit, [code]);
        stopped.add(reader);
      } else if (command.kind === "reclaim") {
        assert.ok(stopped.has(reader));
        const bit = 1 << readers.indexOf(reader);
        if (fault === "clear-all") {
          Atomics.store(words, 0, 0);
          Atomics.store(words, 1, 0);
        } else if (Atomics.and(words, 0, ~bit) & bit) {
          Atomics.sub(words, 1, 1);
        } else if (fault === "double-debit") {
          Atomics.compareExchange(words, 1, 1, 0);
        }
      } else {
        const pending = once(worker, "message");
        worker.postMessage(command.kind);
        const [message] = await pending;
        if (command.kind === "copy") {
          assert.ok(Array.isArray(message));
          assert.equal(message[0], 11);
          assert.ok(message[1] === 12 || message[1] === 22);
          copies[reader] = message[1] === 12 ? "Old" : "Next";
        } else {
          finished.add(reader);
        }
      }
    },
    async observe() {
      const mask = Atomics.load(words, 0);
      const refs = ["Zero", "One", "Two"][Atomics.load(words, 1)];
      assert.ok(refs);
      return {
        pins: readers.filter((_, index) => (mask & (1 << index)) !== 0),
        refs,
        stopped: [...stopped].sort(),
        finished: [...finished].sort(),
        payload: Atomics.load(words, 3) === 12 ? "Old" : "Next",
        copies: { ...copies },
      };
    },
  };
}

test(
  "reclaiming a stopped reader removes only its own reference",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "FirstStoppedWithBothPins",
    );
    assert.deepEqual(base.commands, [{ kind: "stop", reader: "First" }]);
    const reclaim = { kind: "reclaim", reader: "First" } as const;
    const useSlot: Command[] = [
      { kind: "tryReuse" },
      { kind: "copy", reader: "Second" },
    ];
    const commands: Command[] = [
      ...base.commands,
      reclaim,
      reclaim,
      ...useSlot,
      { kind: "finish", reader: "Second" },
      { kind: "tryReuse" },
    ];
    for (const fault of ["none", "clear-all", "double-debit"] as const) {
      await t.test(fault, async (t) => {
        const slot = await openSlot(t, fault);
        const result = await recordReplay(
          checker,
          { ...base, commands },
          {
            ...slot,
            project: ({ pins, refs, stopped, finished, payload, copies }) => ({
              pins,
              refs,
              stopped,
              finished,
              payload,
              copies,
            }),
          },
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[4], {
            pins: ["Second"],
            refs: "One",
            stopped: ["First"],
            finished: [],
            payload: "Old",
            copies: { First: null, Second: null },
          });
          assert.deepEqual(await slot.observe(), {
            pins: [],
            refs: "Zero",
            stopped: ["First"],
            finished: ["Second"],
            payload: "Next",
            copies: { First: null, Second: "Old" },
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, fault === "clear-all" ? 2 : 3);
          for (const command of useSlot) await slot.apply(command);
          assert.deepEqual(await slot.observe(), {
            pins: fault === "clear-all" ? [] : ["Second"],
            refs: "Zero",
            stopped: ["First"],
            finished: [],
            payload: "Next",
            copies: { First: null, Second: "Next" },
          });
        }
      });
    }
  },
);
