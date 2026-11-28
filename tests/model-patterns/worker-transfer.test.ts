import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { MessageChannel, Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Mode = "Copy" | "Move";
type Command =
  | { kind: "send"; mode: Mode }
  | { kind: "deliver" | "editSender" | "editReceiver" };
type Fault = "none" | "copy-instead" | "move-instead" | "deferred-copy";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/worker-transfer/array-buffer.ideal";

function openPort(t: TestContext, fault: Fault) {
  const buffer = new ArrayBuffer(1);
  const sender = new Uint8Array(buffer);
  const alias = new Uint8Array(buffer, 0, 1);
  sender[0] = 61;
  const { port1, port2 } = new MessageChannel();
  t.after(() => port1.close());
  const worker = new Worker(
    `
    const { parentPort, workerData } = require("node:worker_threads");
    let received;
    parentPort.on("message", (kind) => {
      if (kind === "deliver") {
        workerData.once("message", (value) => {
          received = value;
          parentPort.postMessage(received[0]);
        });
      } else if (kind === "editReceiver") {
        received[0] = 92;
        parentPort.postMessage(received[0]);
      }
    });
  `,
    { eval: true, workerData: port2, transferList: [port2] },
  );
  t.after(() => worker.terminate());

  function decode(value: number | undefined) {
    if (value === undefined) return null;
    if (value === 61) return "Initial";
    if (value === 74) return "SenderEdit";
    assert.equal(value, 92);
    return "ReceiverEdit";
  }
  let received: number | undefined;
  return {
    async apply(command: Command) {
      if (command.kind === "send") {
        if (fault === "deferred-copy") return;
        const transfer =
          fault === "move-instead" ||
          (command.mode === "Move" && fault !== "copy-instead");
        port1.postMessage(sender, transfer ? [buffer] : []);
      } else if (command.kind === "editSender") {
        sender[0] = 74;
      } else {
        if (command.kind === "deliver" && fault === "deferred-copy")
          port1.postMessage(sender);
        const response = once(worker, "message");
        worker.postMessage(command.kind);
        [received] = await response;
      }
    },
    async observe() {
      assert.equal(alias.byteLength, buffer.byteLength);
      return {
        sender: decode(sender[0]),
        alias: decode(alias[0]),
        receiver: decode(received),
      };
    },
  };
}

test(
  "worker buffer transfer and copy differ at send time and after delivery",
  { timeout: 30_000 },
  async (t) => {
    const moved = await reachableScenario<Command>(
      checker,
      model,
      "MovedBeforeDelivery",
    );
    const copied = await reachableScenario<Command>(
      checker,
      model,
      "CopiedBeforeDelivery",
    );
    assert.deepEqual(moved.commands, [{ kind: "send", mode: "Move" }]);
    assert.deepEqual(copied.commands, [{ kind: "send", mode: "Copy" }]);
    const cases = [
      [moved, "none"],
      [moved, "copy-instead"],
      [copied, "none"],
      [copied, "move-instead"],
      [copied, "deferred-copy"],
    ] as const;
    for (const [base, fault] of cases) {
      await t.test(
        `${base === moved ? "move" : "copy"} / ${fault}`,
        async (t) => {
          const slot = openPort(t, fault);
          const commands: Command[] = [...base.commands];
          if (base === copied) commands.push({ kind: "editSender" });
          commands.push({ kind: "deliver" }, { kind: "editReceiver" });
          const result = await recordReplay(
            checker,
            { ...base, commands },
            {
              ...slot,
              project: ({ sender, receiver }) => ({
                sender,
                alias: sender,
                receiver,
              }),
            },
          );
          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            const initial = base === moved ? null : "Initial";
            assert.deepEqual(result.recording.observations[1], {
              sender: initial,
              alias: initial,
              receiver: null,
            });
            const sender = base === moved ? null : "SenderEdit";
            assert.deepEqual(await slot.observe(), {
              sender,
              alias: sender,
              receiver: "ReceiverEdit",
            });
          } else {
            assert.equal(result.verdict, "mismatch");
            assert.equal(
              result.difference.step,
              fault === "deferred-copy" ? 3 : 1,
            );
            const sender =
              fault === "copy-instead"
                ? "Initial"
                : fault === "move-instead"
                  ? null
                  : "SenderEdit";
            assert.deepEqual(result.difference.observed, {
              sender,
              alias: sender,
              receiver: fault === "deferred-copy" ? "SenderEdit" : null,
            });
          }
        },
      );
    }
  },
);
