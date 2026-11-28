import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = { kind: "send" | "editReceiver" | "editSender" };
type Fault = "none" | "split-fields" | "merge-equal-objects";
type Field = "Left" | "Right" | "Twin";
type Value = "Original" | "Edited";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/worker-transfer/shared-references.model";
const fields = ["Left", "Right", "Twin"] as const;

function openWorker(t: TestContext, fault: Fault) {
  const shared: { value: Value } = { value: "Original" };
  const sender = {
    Left: shared,
    Right: shared,
    Twin: { value: "Original" as Value },
  };
  const worker = new Worker(
    `
    const { parentPort, workerData: fault } = require("node:worker_threads");
    let received;
    parentPort.on("message", (command) => {
      if (command.kind === "send") {
        received = command.message;
        if (fault === "split-fields") received.Right = structuredClone(received.Right);
        if (fault === "merge-equal-objects") received.Twin = received.Left;
      } else if (command.kind === "editReceiver") {
        received.Left.value = "Edited";
      }
      parentPort.postMessage(received ? {
        Left: received.Left.value, Right: received.Right.value, Twin: received.Twin.value,
        alias: received.Left === received.Right,
        distinctTwin: received.Left !== received.Twin,
      } : null);
    });
  `,
    { eval: true, workerData: fault },
  );
  t.after(() => worker.terminate());
  let received:
    | (Record<Field, Value> & { alias: boolean; distinctTwin: boolean })
    | null = null;
  return {
    async apply({ kind }: Command) {
      if (kind === "editSender") {
        sender.Twin.value = "Edited";
        return;
      }
      const response = once(worker, "message");
      worker.postMessage(
        kind === "send" ? { kind, message: sender } : { kind },
      );
      [received] = await response;
    },
    async observe() {
      return {
        sender: {
          Left: sender.Left.value,
          Right: sender.Right.value,
          Twin: sender.Twin.value,
        },
        received,
      };
    },
  };
}

test(
  "structured copy keeps internal aliases and keeps equal but distinct objects separate",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "MessageCopied",
    );
    assert.deepEqual(base.commands, [{ kind: "send" }]);
    const tail: Command[] = [{ kind: "editReceiver" }, { kind: "editSender" }];
    for (const fault of [
      "none",
      "split-fields",
      "merge-equal-objects",
    ] as const) {
      await t.test(fault, async (t) => {
        const adapter = openWorker(t, fault);
        const result = await recordReplay(
          checker,
          { ...base, commands: [...base.commands, ...tail] },
          {
            ...adapter,
            project({ source, target, values }) {
              const originals = source as Record<Field, string>;
              const copies = target as Record<Field, string | null>;
              const bytes = values as Record<string, Value>;
              return {
                sender: Object.fromEntries(
                  fields.map((field) => [field, bytes[originals[field]]]),
                ),
                received:
                  copies.Left === null
                    ? null
                    : {
                        Left: bytes[copies.Left],
                        Right:
                          copies.Right === null ? null : bytes[copies.Right],
                        Twin: copies.Twin === null ? null : bytes[copies.Twin],
                        alias: copies.Left === copies.Right,
                        distinctTwin: copies.Left !== copies.Twin,
                      },
              };
            },
          },
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, 1);
          for (const command of tail) await adapter.apply(command);
        }
        assert.deepEqual(await adapter.observe(), {
          sender: { Left: "Original", Right: "Original", Twin: "Edited" },
          received: {
            Left: "Edited",
            Right: fault === "split-fields" ? "Original" : "Edited",
            Twin: fault === "merge-equal-objects" ? "Edited" : "Original",
            alias: fault !== "split-fields",
            distinctTwin: fault !== "merge-equal-objects",
          },
        });
      });
    }
  },
);
