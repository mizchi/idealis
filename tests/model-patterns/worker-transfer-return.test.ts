import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { MessageChannel, Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = {
  kind:
    | "send"
    | "acceptAtWorker"
    | "editAtWorker"
    | "sendBack"
    | "acceptAtHost"
    | "editAtHost";
};
type Fault = "none" | "copy-return" | "rebind-old-handle";
type Byte = "Initial" | "Updated" | "Final";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/worker-transfer/round-trip.model";

function openPort(t: TestContext, fault: Fault) {
  const original = new Uint8Array([61]);
  let originalHandle: Uint8Array = original;
  let returned: Uint8Array | undefined;
  const { port1, port2 } = new MessageChannel();
  t.after(() => port1.close());
  const worker = new Worker(
    `
    const { parentPort, workerData: { port, copy } } = require("node:worker_threads");
    let view;
    parentPort.on("message", (kind) => {
      if (kind === "inspect") {
        parentPort.postMessage(view?.[0]);
      } else if (kind === "acceptAtWorker") {
        port.once("message", (bytes) => {
          view = bytes;
          parentPort.postMessage(view[0]);
        });
      } else if (kind === "editAtWorker") {
        view[0] = 74;
        parentPort.postMessage(view[0]);
      } else if (kind === "sendBack") {
        port.postMessage(view, copy ? [] : [view.buffer]);
        parentPort.postMessage(view[0]);
      }
    });
  `,
    {
      eval: true,
      workerData: { port: port2, copy: fault === "copy-return" },
      transferList: [port2],
    },
  );
  t.after(() => worker.terminate());
  const returnedMessage = once(port1, "message");
  function decode(byte: number | undefined): Byte | null {
    if (byte === undefined) return null;
    if (byte === 61) return "Initial";
    if (byte === 74) return "Updated";
    assert.equal(byte, 92);
    return "Final";
  }
  return {
    async apply({ kind }: Command) {
      if (kind === "send") {
        port1.postMessage(original, [original.buffer]);
        assert.equal(original.byteLength, 0);
      } else if (kind === "acceptAtHost") {
        const [incoming] = await returnedMessage;
        assert.ok(incoming instanceof Uint8Array);
        assert.notEqual(incoming.buffer, original.buffer);
        assert.equal(original.byteLength, 0);
        returned = incoming;
        if (fault === "rebind-old-handle") originalHandle = incoming;
      } else if (kind === "editAtHost") {
        assert.ok(returned);
        returned[0] = 92;
      } else {
        const response = once(worker, "message");
        worker.postMessage(kind);
        await response;
      }
    },
    async observe() {
      const response = once(worker, "message");
      worker.postMessage("inspect");
      const [byte] = await response;
      return {
        original: decode(originalHandle[0]),
        workerView: decode(byte),
        returned: decode(returned?.[0]),
      };
    },
  };
}

test(
  "a buffer can make a round trip without restoring the relinquished views",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "ReturnedUpdated",
    );
    assert.deepEqual(base.commands, [
      { kind: "send" },
      { kind: "acceptAtWorker" },
      { kind: "editAtWorker" },
      { kind: "sendBack" },
      { kind: "acceptAtHost" },
    ]);
    const commands: Command[] = [...base.commands, { kind: "editAtHost" }];
    for (const fault of ["none", "copy-return", "rebind-old-handle"] as const) {
      await t.test(fault, async (t) => {
        const adapter = openPort(t, fault);
        const result = await recordReplay(
          checker,
          { ...base, commands },
          {
            ...adapter,
            project: ({ original, workerView, returned }) => ({
              original,
              workerView,
              returned,
            }),
          },
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[5], {
            original: null,
            workerView: null,
            returned: "Updated",
          });
          assert.deepEqual(await adapter.observe(), {
            original: null,
            workerView: null,
            returned: "Final",
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, fault === "copy-return" ? 4 : 5);
          assert.deepEqual(
            await adapter.observe(),
            fault === "copy-return"
              ? { original: null, workerView: "Updated", returned: null }
              : { original: "Updated", workerView: null, returned: "Updated" },
          );
        }
      });
    }
  },
);
