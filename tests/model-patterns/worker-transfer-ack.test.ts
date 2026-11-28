import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { MessageChannel, Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = { kind: "send" | "deliver" | "acknowledge" | "close" };
type Fault = "none" | "send-is-ack" | "close-means-unprocessed";
type Status = "Idle" | "Pending" | "Confirmed" | "Unknown" | "Failed";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/worker-transfer/acknowledgment.model";

function openPort(t: TestContext, fault: Fault) {
  const source = new Uint8Array([61]);
  const { port1, port2 } = new MessageChannel();
  t.after(() => port1.close());
  const worker = new Worker(
    `
    const { parentPort, workerData: port } = require("node:worker_threads");
    let delivered;
    parentPort.on("message", (kind) => {
      if (kind === "deliver") {
        port.once("message", (bytes) => {
          delivered = bytes[0];
          parentPort.postMessage(delivered);
        });
      } else if (kind === "acknowledge") {
        port.postMessage({ ack: delivered });
      } else if (kind === "close") {
        port.once("close", () => {
          parentPort.postMessage(delivered);
        });
        port.close();
      }
    });
  `,
    { eval: true, workerData: port2, transferList: [port2] },
  );
  t.after(() => worker.terminate());
  let open = true;
  let status: Status = "Idle";
  let delivered = false;
  let ack = false;
  return {
    async apply({ kind }: Command) {
      switch (kind) {
        case "send":
          port1.postMessage(source, [source.buffer]);
          status = fault === "send-is-ack" ? "Confirmed" : "Pending";
          break;
        case "deliver": {
          const probe = once(worker, "message");
          worker.postMessage(kind);
          assert.deepEqual(await probe, [61]);
          delivered = true;
          break;
        }
        case "acknowledge": {
          const receipt = once(port1, "message");
          worker.postMessage(kind);
          assert.deepEqual(await receipt, [{ ack: 61 }]);
          ack = true;
          status = "Confirmed";
          break;
        }
        case "close": {
          const closed = once(port1, "close");
          const probe = once(worker, "message");
          worker.postMessage(kind);
          assert.deepEqual(await probe, [delivered ? 61 : undefined]);
          await closed;
          open = false;
          if (status === "Pending")
            status = fault === "close-means-unprocessed" ? "Failed" : "Unknown";
          break;
        }
      }
    },
    async observe() {
      return { open, source: source[0] === 61, delivered, ack, status };
    },
    verifySourceIsDetached() {
      assert.equal(source.byteLength, 0);
      assert.throws(
        () => structuredClone(source, { transfer: [source.buffer] }),
        { name: "DataCloneError" },
      );
    },
  };
}

test(
  "closing a transfer port before an acknowledgment leaves the sender uncertain",
  { timeout: 30_000 },
  async (t) => {
    const before = await reachableScenario<Command>(
      checker,
      model,
      "ClosedBeforeDelivery",
    );
    const after = await reachableScenario<Command>(
      checker,
      model,
      "ClosedAfterDeliveryWithoutAck",
    );
    const confirmed = await reachableScenario<Command>(
      checker,
      model,
      "ConfirmedThenClosed",
    );
    assert.deepEqual(before.commands, [{ kind: "send" }, { kind: "close" }]);
    assert.deepEqual(after.commands, [
      { kind: "send" },
      { kind: "deliver" },
      { kind: "close" },
    ]);
    assert.deepEqual(confirmed.commands, [
      { kind: "send" },
      { kind: "deliver" },
      { kind: "acknowledge" },
      { kind: "close" },
    ]);
    for (const [scenario, fault] of [
      [before, "none"],
      [after, "none"],
      [confirmed, "none"],
      [before, "send-is-ack"],
      [after, "close-means-unprocessed"],
    ] as const) {
      await t.test(
        `${scenario.commands.length} commands / ${fault}`,
        async (t) => {
          const adapter = openPort(t, fault);
          const result = await recordReplay(checker, scenario, {
            ...adapter,
            project: ({ open, source, delivered, ack, status }) => ({
              open,
              source,
              delivered,
              ack,
              status,
            }),
          });
          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(await adapter.observe(), {
              open: false,
              source: false,
              delivered: scenario !== before,
              ack: scenario === confirmed,
              status: scenario === confirmed ? "Confirmed" : "Unknown",
            });
          } else {
            assert.equal(result.verdict, "mismatch");
            assert.equal(
              result.difference.step,
              fault === "send-is-ack" ? 1 : 3,
            );
            assert.deepEqual(await adapter.observe(), {
              open: fault === "send-is-ack",
              source: false,
              delivered: fault === "close-means-unprocessed",
              ack: false,
              status: fault === "send-is-ack" ? "Confirmed" : "Failed",
            });
          }
          adapter.verifySourceIsDetached();
        },
      );
    }
  },
);
