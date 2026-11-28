import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import {
  markAsUntransferable,
  MessageChannel,
  Worker,
} from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Issue = "Duplicate" | "Uncloneable" | "Blocked" | "Valid";
type Command = { kind: "send"; issue: Issue } | { kind: "poll" };
type Fault = "none" | "detach-too-soon" | "enqueue-too-soon";
type Value = "LeftByte" | "RightByte";
const model = "examples/model-patterns/worker-transfer/rejection.model";
const checker = "crates/rcheck/target/debug/rcheck";

function openPort(t: TestContext, issue: Issue, fault: Fault) {
  const left = new Uint8Array([61]);
  const right = new Uint8Array([92]);
  if (issue === "Blocked") markAsUntransferable(right.buffer);
  const { port1, port2 } = new MessageChannel();
  t.after(() => port1.close());
  const worker = new Worker(
    `
    const { parentPort, workerData } = require("node:worker_threads");
    let queued = [];
    workerData.on("message", (message) => {
      if (message === "barrier") {
        parentPort.postMessage(queued);
        queued = [];
      } else queued.push([message.left[0], message.right[0]]);
    });
  `,
    { eval: true, workerData: port2, transferList: [port2] },
  );
  t.after(() => worker.terminate());
  let result: "Rejected" | "Accepted" | null = null;
  let receiver: { Left: Value | null; Right: Value | null } = {
    Left: null,
    Right: null,
  };
  function value(byte: number | undefined): Value | null {
    if (byte === undefined) return null;
    if (byte === 61) return "LeftByte";
    assert.equal(byte, 92);
    return "RightByte";
  }
  return {
    async apply(command: Command) {
      if (command.kind === "poll") {
        const reply = once(worker, "message");
        port1.postMessage("barrier");
        const [delivered] = await reply;
        assert.ok(Array.isArray(delivered) && delivered.length <= 1);
        if (delivered.length === 1) {
          const [a, b] = delivered[0];
          receiver = { Left: value(a), Right: value(b) };
        }
        return;
      }
      const message = { left, right };
      if (fault === "enqueue-too-soon" && command.issue !== "Valid")
        port1.postMessage(message);
      if (fault === "detach-too-soon" && command.issue !== "Valid")
        structuredClone(null, { transfer: [left.buffer] });
      const transfer =
        command.issue === "Duplicate"
          ? [left.buffer, left.buffer]
          : command.issue === "Blocked"
            ? [left.buffer, right.buffer]
            : [left.buffer];
      try {
        port1.postMessage(
          command.issue === "Uncloneable"
            ? { ...message, callback() {} }
            : message,
          transfer,
        );
        result = "Accepted";
      } catch (error) {
        if (!(error instanceof DOMException) || error.name !== "DataCloneError")
          throw error;
        result = "Rejected";
      }
    },
    async observe() {
      return {
        held: { Left: value(left[0]), Right: value(right[0]) },
        receiver,
        result,
      };
    },
  };
}

test(
  "a rejected transfer keeps both source buffers and enqueues no delivery",
  { timeout: 30_000 },
  async (t) => {
    for (const [issue, fault] of [
      ["Duplicate", "none"],
      ["Uncloneable", "none"],
      ["Blocked", "none"],
      ["Duplicate", "detach-too-soon"],
      ["Uncloneable", "enqueue-too-soon"],
    ] as const) {
      await t.test(`${issue} / ${fault}`, async (t) => {
        const base = await reachableScenario<Command>(
          checker,
          model,
          `Rejected${issue}`,
        );
        assert.deepEqual(base.commands, [{ kind: "send", issue }]);
        const commands: Command[] = [
          ...base.commands,
          { kind: "poll" },
          { kind: "send", issue: "Valid" },
          { kind: "poll" },
        ];
        const adapter = openPort(t, issue, fault);
        const result = await recordReplay(
          checker,
          { ...base, commands },
          {
            ...adapter,
            project: ({ held, receiver, result }) => ({
              held,
              receiver,
              result,
            }),
          },
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[2], {
            held: { Left: "LeftByte", Right: "RightByte" },
            receiver: { Left: null, Right: null },
            result: "Rejected",
          });
          assert.deepEqual(await adapter.observe(), {
            held: { Left: null, Right: "RightByte" },
            receiver: { Left: "LeftByte", Right: "RightByte" },
            result: "Accepted",
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(
            result.difference.step,
            fault === "detach-too-soon" ? 1 : 2,
          );
          assert.deepEqual(await adapter.observe(), {
            held: {
              Left: fault === "detach-too-soon" ? null : "LeftByte",
              Right: "RightByte",
            },
            receiver:
              fault === "enqueue-too-soon"
                ? { Left: "LeftByte", Right: "RightByte" }
                : { Left: null, Right: null },
            result: "Rejected",
          });
        }
      });
    }
  },
);
