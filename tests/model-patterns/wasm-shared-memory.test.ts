import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command =
  | { kind: "borrow" }
  | { kind: "growAndWrite" }
  | { kind: "read"; region: "Prefix" | "Suffix" };
type Fault = "none" | "keep-short-view" | "copy-prefix";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model =
  "examples/model-patterns/wasm-memory/shared-view-after-grow.ideal";
const page = 65_536;
const prefixOffset = 16;
const suffixOffset = page + prefixOffset;

function openMemory(t: TestContext, fault: Fault) {
  const memory = new WebAssembly.Memory({
    initial: 1,
    maximum: 2,
    shared: true,
  });
  const first = memory.buffer as ArrayBufferLike;
  assert.ok(first instanceof SharedArrayBuffer);
  Atomics.store(new Uint8Array(first), prefixOffset, 61);
  const writer = new Worker(
    `
    const { parentPort, workerData: { memory, prefixOffset, suffixOffset } } = require("node:worker_threads");
    parentPort.once("message", () => {
      const previous = memory.grow(1);
      const view = new Uint8Array(memory.buffer);
      Atomics.store(view, prefixOffset, 74);
      Atomics.store(view, suffixOffset, 92);
      parentPort.postMessage(previous);
    });
  `,
    { eval: true, workerData: { memory, prefixOffset, suffixOffset } },
  );
  t.after(() => writer.terminate());

  let view: Uint8Array | undefined;
  let value: "Original" | "Updated" | "Added" | null = null;
  function extent(length: number) {
    if (length === page) return "FirstPage";
    assert.equal(length, 2 * page);
    return "BothPages";
  }
  const decode = (byte: number | undefined) => {
    if (byte === undefined) return null;
    if (byte === 61) return "Original";
    if (byte === 74) return "Updated";
    assert.equal(byte, 92);
    return "Added";
  };
  return {
    async apply(command: Command) {
      if (command.kind === "borrow") {
        view = new Uint8Array(memory.buffer);
        if (fault === "copy-prefix") view = view.slice();
      } else if (command.kind === "growAndWrite") {
        const result = once(writer, "message");
        writer.postMessage(null);
        assert.deepEqual(await result, [1]);
        assert.equal(first.byteLength, page);
        assert.equal(memory.buffer.byteLength, 2 * page);
      } else {
        assert.ok(view);
        const offset =
          command.region === "Prefix" ? prefixOffset : suffixOffset;
        if (
          offset >= view.length &&
          offset < memory.buffer.byteLength &&
          fault !== "keep-short-view"
        )
          view = new Uint8Array(memory.buffer);
        value = decode(
          offset < view.length ? Atomics.load(view, offset) : undefined,
        );
      }
    },
    async observe() {
      const current = new Uint8Array(memory.buffer);
      return {
        pages: extent(current.length),
        prefix: decode(Atomics.load(current, prefixOffset)),
        suffix: decode(
          current.length > suffixOffset
            ? Atomics.load(current, suffixOffset)
            : undefined,
        ),
        view: view === undefined ? null : extent(view.length),
        value,
      };
    },
  };
}

test(
  "a fixed shared Wasm view keeps aliasing the prefix and can be refreshed for the suffix",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "OldViewAfterGrowth",
    );
    assert.deepEqual(base.commands, [
      { kind: "borrow" },
      { kind: "growAndWrite" },
    ]);
    const commands = [...base.commands];
    commands.splice(
      1,
      0,
      { kind: "read", region: "Prefix" },
      { kind: "read", region: "Suffix" },
    );
    commands.push(
      { kind: "read", region: "Prefix" },
      { kind: "read", region: "Suffix" },
    );
    for (const fault of ["none", "keep-short-view", "copy-prefix"] as const) {
      await t.test(fault, async (t) => {
        const memory = openMemory(t, fault);
        const result = await recordReplay(
          checker,
          { ...base, commands },
          {
            ...memory,
            project: ({ pages, prefix, suffix, view, value }) => ({
              pages,
              prefix,
              suffix,
              view,
              value,
            }),
          },
        );
        assert.deepEqual(result.recording.observations[3], {
          pages: "FirstPage",
          prefix: "Original",
          suffix: null,
          view: "FirstPage",
          value: null,
        });
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[5], {
            pages: "BothPages",
            prefix: "Updated",
            suffix: "Added",
            view: "FirstPage",
            value: "Updated",
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, fault === "copy-prefix" ? 5 : 6);
        }
        assert.deepEqual(await memory.observe(), {
          pages: "BothPages",
          prefix: "Updated",
          suffix: "Added",
          view: fault === "none" ? "BothPages" : "FirstPage",
          value:
            fault === "none"
              ? "Added"
              : fault === "copy-prefix"
                ? "Original"
                : null,
        });
      });
    }
  },
);
