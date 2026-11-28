import assert from "node:assert/strict";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";
import { openGrowMemory } from "./helpers/wasm-grow.js";

type Command =
  | { kind: "borrow" | "read" }
  | { kind: "grow"; delta: "Zero" | "One" };
type Fault = "none" | "keep-detached" | "wrong-offset";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/wasm-memory/view-after-grow.model";
const page = 65_536;
const address = { Payload: 16, Adjacent: 17 } as const;

function openMemory(fault: Fault) {
  const { memory, grow } = openGrowMemory();
  const first = memory.buffer;
  const initial = new Uint8Array(first);
  initial[address.Payload] = 61;
  initial[address.Adjacent] = 92;
  let cached: Uint8Array | undefined;
  let used: keyof typeof address | null = null;
  let value: "Expected" | "Neighbor" | null = null;
  return {
    async apply(command: Command) {
      if (command.kind === "borrow") {
        cached = new Uint8Array(memory.buffer);
      } else if (command.kind === "grow") {
        assert.equal(grow(command.delta === "Zero" ? 0 : 1), 1);
        assert.notEqual(memory.buffer, first);
        assert.equal(first.byteLength, 0);
      } else {
        assert.ok(cached);
        const changed = cached.buffer !== memory.buffer;
        if (changed && fault !== "keep-detached")
          cached = new Uint8Array(memory.buffer);
        used = changed && fault === "wrong-offset" ? "Adjacent" : "Payload";
        const byte = cached[address[used]];
        if (byte === undefined) value = null;
        else if (byte === 61) value = "Expected";
        else {
          assert.equal(byte, 92);
          value = "Neighbor";
        }
      }
    },
    async observe() {
      return {
        epoch: memory.buffer === first ? "Initial" : "Refreshed",
        expanded: memory.buffer.byteLength === 2 * page,
        view:
          cached === undefined
            ? null
            : cached.buffer === first
              ? "Initial"
              : "Refreshed",
        used,
        value,
      };
    },
  };
}

test("Wasm memory growth invalidates a fixed view even when page count stays the same", async (t) => {
  for (const [delta, target] of [
    ["Zero", "DetachedWithoutMorePages"],
    ["One", "DetachedWithMorePages"],
  ] as const) {
    const base = await reachableScenario<Command>(checker, model, target);
    assert.deepEqual(base.commands, [
      { kind: "borrow" },
      { kind: "grow", delta },
    ]);
    const commands = [...base.commands];
    commands.splice(1, 0, { kind: "read" });
    commands.push({ kind: "read" });
    for (const fault of ["none", "keep-detached", "wrong-offset"] as const) {
      await t.test(`${delta} / ${fault}`, async () => {
        const memory = openMemory(fault);
        const result = await recordReplay(
          checker,
          { ...base, commands },
          {
            ...memory,
            project: ({ epoch, expanded, view, used, value }) => ({
              epoch,
              expanded,
              view,
              used,
              value,
            }),
          },
        );
        assert.deepEqual(result.recording.observations[2], {
          epoch: "Initial",
          expanded: false,
          view: "Initial",
          used: "Payload",
          value: "Expected",
        });
        if (fault === "none") assert.equal(result.verdict, "matched");
        else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, 4);
        }
        assert.deepEqual(await memory.observe(), {
          epoch: "Refreshed",
          expanded: delta === "One",
          view: fault === "keep-detached" ? "Initial" : "Refreshed",
          used: fault === "wrong-offset" ? "Adjacent" : "Payload",
          value:
            fault === "keep-detached"
              ? null
              : fault === "wrong-offset"
                ? "Neighbor"
                : "Expected",
        });
      });
    }
  }
});
