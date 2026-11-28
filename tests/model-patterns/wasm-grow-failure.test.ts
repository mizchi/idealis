import assert from "node:assert/strict";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";
import { openGrowMemory } from "./helpers/wasm-grow.js";

type Entry = "Instruction" | "Host";
type Command =
  | { kind: "borrow" }
  | { kind: "read" }
  | { kind: "attempt"; entry: Entry; amount: "One" | "Two" };
type Fault = "none" | "discard-on-failure" | "collapse-host-error";
type Result = "Grown" | "MinusOne" | "RangeFailure";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/wasm-memory/grow-failure.model";
const page = 65_536;
const address = 17;

function adapter(fault: Fault) {
  const { memory, grow } = openGrowMemory();
  const first = memory.buffer;
  new Uint8Array(first)[address] = 93;
  let view: Uint8Array | undefined;
  let result: Result | null = null;
  let byteRead = false;
  return {
    async apply(command: Command) {
      if (command.kind === "borrow") {
        view = new Uint8Array(memory.buffer);
      } else if (command.kind === "read") {
        if (view && view.buffer !== memory.buffer)
          view = new Uint8Array(memory.buffer);
        byteRead = view?.[address] === 93;
      } else {
        const buffer = memory.buffer;
        const previousPages = buffer.byteLength / page;
        const amount = command.amount === "One" ? 1 : 2;
        if (command.entry === "Instruction") {
          const returned = grow(amount);
          assert.ok(returned === -1 || returned === previousPages);
          result = returned === -1 ? "MinusOne" : "Grown";
        } else {
          try {
            assert.equal(memory.grow(amount), previousPages);
            result = "Grown";
          } catch (error) {
            if (!(error instanceof RangeError)) throw error;
            result =
              fault === "collapse-host-error" ? "MinusOne" : "RangeFailure";
          }
        }
        if (result !== "Grown") {
          assert.equal(memory.buffer, buffer);
          assert.equal(buffer.byteLength, previousPages * page);
          if (fault === "discard-on-failure") view = undefined;
        }
      }
    },
    async observe() {
      const buffer = memory.buffer;
      return {
        pages: buffer.byteLength === page ? "FirstPage" : "BothPages",
        epoch: buffer === first ? "Initial" : "Refreshed",
        view:
          view === undefined
            ? null
            : view.buffer === first
              ? "Initial"
              : "Refreshed",
        result,
        byteRead,
      };
    },
  };
}

test("failed Wasm growth preserves the cached view and distinguishes both APIs", async (t) => {
  for (const [entry, question, failure] of [
    ["Instruction", "InstructionFailedWithView", "MinusOne"],
    ["Host", "HostFailedWithView", "RangeFailure"],
  ] as const) {
    const base = await reachableScenario<Command>(checker, model, question);
    assert.deepEqual(base.commands, [
      { kind: "borrow" },
      { kind: "attempt", entry, amount: "Two" },
    ]);
    const commands: Command[] = [
      ...base.commands,
      { kind: "read" },
      { kind: "attempt", entry, amount: "One" },
      { kind: "read" },
      { kind: "attempt", entry, amount: "One" },
      { kind: "read" },
    ];
    const faults: Fault[] = ["none", "discard-on-failure"];
    if (entry === "Host") faults.push("collapse-host-error");
    for (const fault of faults) {
      await t.test(`${entry} / ${fault}`, async () => {
        const memory = adapter(fault);
        const replay = await recordReplay(
          checker,
          { ...base, commands },
          {
            ...memory,
            project: ({ pages, epoch, view, result, byteRead }) => ({
              pages,
              epoch,
              view,
              result,
              byteRead,
            }),
          },
        );
        if (fault === "none") {
          assert.equal(replay.verdict, "matched");
          assert.deepEqual(replay.recording.observations[3], {
            pages: "FirstPage",
            epoch: "Initial",
            view: "Initial",
            result: failure,
            byteRead: true,
          });
          assert.deepEqual(await memory.observe(), {
            pages: "BothPages",
            epoch: "Refreshed",
            view: "Refreshed",
            result: failure,
            byteRead: true,
          });
        } else {
          assert.equal(replay.verdict, "mismatch");
          assert.equal(replay.difference.step, 2);
          await memory.apply({ kind: "read" });
          assert.deepEqual(await memory.observe(), {
            pages: "FirstPage",
            epoch: "Initial",
            view: fault === "discard-on-failure" ? null : "Initial",
            result: fault === "collapse-host-error" ? "MinusOne" : failure,
            byteRead: fault !== "discard-on-failure",
          });
        }
      });
    }
  }
});
