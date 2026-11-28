import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const readers = ["Current", "Delayed"] as const;
const slots = ["Left", "Right"] as const;
const tickets = ["First", "Second", "Third"] as const;
type Reader = (typeof readers)[number];
type Ticket = (typeof tickets)[number];
type Command =
  | { kind: "snapshot"; reader: Reader; slot: (typeof slots)[number] }
  | { kind: "claim"; reader: Reader }
  | { kind: "publishThird" };

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/worker-deque/ring-reuse.model";

function openRing(t: TestContext, ignoreTicket: boolean) {
  const words = new Int32Array(
    new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * slots.length),
  );
  words.set([1, 2]);
  function start() {
    const worker = new Worker(
      `
    const { parentPort, workerData } = require("node:worker_threads");
    const { words, ignoreTicket } = workerData;
    let slot;
    let ticket;
    parentPort.on("message", (command) => {
      if (command.kind === "snapshot") {
        slot = command.index;
        ticket = Atomics.load(words, slot);
        parentPort.postMessage(ticket);
      } else {
        const prior = ignoreTicket ? Atomics.exchange(words, slot, 0)
          : Atomics.compareExchange(words, slot, ticket, 0);
        const result = ignoreTicket || prior === ticket ? prior : 0;
        ticket = undefined;
        parentPort.postMessage(result);
      }
    });
    `,
      { eval: true, workerData: { words, ignoreTicket } },
    );
    t.after(() => worker.terminate());
    return worker;
  }
  const workers = { Current: start(), Delayed: start() };
  const observed: Record<Reader, Ticket | null> = {
    Current: null,
    Delayed: null,
  };
  const claimed: Record<Ticket, Reader[]> = {
    First: [],
    Second: [],
    Third: [],
  };
  const published: Ticket[] = ["First", "Second"];
  const decode = (word: number) => tickets[word - 1] ?? null;
  return {
    async apply(command: Command) {
      if (command.kind === "publishThird") {
        assert.equal(Atomics.compareExchange(words, 0, 0, 3), 0);
        published.push("Third");
        return;
      }
      const worker = workers[command.reader];
      const response = once(worker, "message");
      worker.postMessage(
        command.kind === "snapshot"
          ? { kind: "snapshot", index: slots.indexOf(command.slot) }
          : { kind: "claim" },
      );
      const [word] = await response;
      assert.ok(Number.isInteger(word) && word >= 0 && word <= tickets.length);
      const ticket = decode(word);
      if (command.kind === "snapshot") {
        assert.ok(ticket);
        observed[command.reader] = ticket;
      } else {
        observed[command.reader] = null;
        if (ticket) claimed[ticket].push(command.reader);
      }
    },
    async observe() {
      return {
        cells: {
          Left: decode(Atomics.load(words, 0)),
          Right: decode(Atomics.load(words, 1)),
        },
        observed: { ...observed },
        claimed: {
          First: [...claimed.First],
          Second: [...claimed.Second],
          Third: [...claimed.Third],
        },
        published: [...published],
      };
    },
  };
}

test(
  "reusing a ring slot does not let an old reader claim the next ticket",
  { timeout: 30_000 },
  async (t) => {
    const stale = await reachableScenario<Command>(
      checker,
      model,
      "WrappedWithOldCandidate",
    );
    assert.equal(stale.commands.length, 4);
    for (const ignoreTicket of [false, true]) {
      await t.test(
        ignoreTicket ? "compare slot only" : "compare slot and ticket",
        async (t) => {
          const ring = openRing(t, ignoreTicket);
          const commands: Command[] = [
            ...stale.commands,
            { kind: "claim", reader: "Delayed" },
            { kind: "snapshot", reader: "Delayed", slot: "Left" },
            { kind: "claim", reader: "Delayed" },
            { kind: "snapshot", reader: "Current", slot: "Right" },
            { kind: "claim", reader: "Current" },
          ];
          const result = await recordReplay(
            checker,
            { ...stale, commands },
            {
              ...ring,
              project: ({ cells, observed, claimed, published }) => ({
                cells,
                observed,
                claimed,
                published,
              }),
            },
          );
          assert.deepEqual(result.recording.observations[4], {
            cells: { Left: "Third", Right: "Second" },
            observed: { Current: null, Delayed: "First" },
            claimed: { First: ["Current"], Second: [], Third: [] },
            published: ["First", "Second", "Third"],
          });
          if (ignoreTicket) {
            assert.equal(result.verdict, "mismatch");
            assert.equal(result.difference.step, 5);
            assert.deepEqual(result.difference.observed, {
              cells: { Left: null, Right: "Second" },
              observed: { Current: null, Delayed: null },
              claimed: { First: ["Current"], Second: [], Third: ["Delayed"] },
              published: ["First", "Second", "Third"],
            });
          } else {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(result.recording.observations[5], {
              cells: { Left: "Third", Right: "Second" },
              observed: { Current: null, Delayed: null },
              claimed: { First: ["Current"], Second: [], Third: [] },
              published: ["First", "Second", "Third"],
            });
            assert.deepEqual(await ring.observe(), {
              cells: { Left: null, Right: null },
              observed: { Current: null, Delayed: null },
              claimed: {
                First: ["Current"],
                Second: ["Current"],
                Third: ["Delayed"],
              },
              published: ["First", "Second", "Third"],
            });
          }
        },
      );
    }
  },
);
