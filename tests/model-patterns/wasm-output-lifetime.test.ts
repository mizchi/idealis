import assert from "node:assert/strict";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Output = "First" | "Second";
type Command = { kind: "dispatch"; output: Output } | { kind: "retain" };

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/wasm-memory/output-lifetime.model";

// (module (memory (export "memory") 1 1)
//   (func (export "dispatch") (param i32) (result i32)
//     i32.const 16 local.get 0 i32.store8 i32.const 16))
const moduleBytes = Uint8Array.from(
  Buffer.from(
    [
      "0061736d01000000",
      "01060160017f017f",
      "03020100",
      "050401010101",
      "071502066d656d6f727902000864697370617463680000",
      "0a0d010b00411020003a000041100b",
    ].join(""),
    "hex",
  ),
);

function openAbi(keepBorrowedView: boolean) {
  const instance = new WebAssembly.Instance(
    new WebAssembly.Module(moduleBytes),
  );
  const memory = instance.exports.memory as WebAssembly.Memory;
  const dispatch = instance.exports.dispatch as (value: number) => number;
  const buffer = memory.buffer;
  let output: Uint8Array | undefined;
  let retained: Uint8Array | undefined;
  let captured: Output | null = null;
  function decode(bytes: Uint8Array | undefined): Output | null {
    if (!bytes) return null;
    if (bytes[0] === 61) return "First";
    assert.equal(bytes[0], 92);
    return "Second";
  }
  return {
    async apply(command: Command) {
      if (command.kind === "dispatch") {
        const pointer = dispatch(command.output === "First" ? 61 : 92);
        assert.equal(pointer, 16);
        assert.equal(memory.buffer, buffer);
        output = new Uint8Array(memory.buffer, pointer, 1);
      } else {
        assert.ok(output);
        captured = decode(output);
        retained = keepBorrowedView ? output : output.slice();
      }
    },
    async observe() {
      assert.equal(memory.buffer, buffer);
      return { current: decode(output), captured, retained: decode(retained) };
    },
  };
}

test("a retained Wasm output survives reuse of its address by the next dispatch", async (t) => {
  const final = await reachableScenario<Command>(
    checker,
    model,
    "RetainedFirstThenDispatchedSecond",
  );
  assert.deepEqual(final.commands, [
    { kind: "dispatch", output: "First" },
    { kind: "retain" },
    { kind: "dispatch", output: "Second" },
  ]);
  for (const keepBorrowedView of [false, true]) {
    await t.test(
      keepBorrowedView ? "retain the view" : "retain a copy",
      async () => {
        const adapter = openAbi(keepBorrowedView);
        const result = await recordReplay(checker, final, {
          ...adapter,
          project: ({ current, captured, retained }) => ({
            current,
            captured,
            retained,
          }),
        });
        assert.deepEqual(result.recording.observations[2], {
          current: "First",
          captured: "First",
          retained: "First",
        });
        if (keepBorrowedView) {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, 3);
        } else {
          assert.equal(result.verdict, "matched");
        }
        assert.deepEqual(await adapter.observe(), {
          current: "Second",
          captured: "First",
          retained: keepBorrowedView ? "Second" : "First",
        });
      },
    );
  }
});
