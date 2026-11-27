import assert from "node:assert/strict";
import test, { type TestContext } from "node:test";
import { MessageChannel, receiveMessageOnPort } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Epoch = "Original" | "Renewed";
type Position = "One" | "Two";
type Frame = { stream: Epoch; position: Position };
type Command =
  | { kind: "seal" | "recycle" }
  | { kind: "drop" | "request" | "resend"; position: Position }
  | ({ kind: "arrive" } & Frame);
type Fault = "count-only" | "ignore-epoch" | null;

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/delivery-ticket-gap.model";
const positions = ["One", "Two"] as const;

function openDelivery(t: TestContext, fault: Fault) {
  const data = new MessageChannel();
  const control = new MessageChannel();
  t.after(() => {
    data.port1.close();
    data.port2.close();
    control.port1.close();
    control.port2.close();
  });
  let epoch: Epoch = "Original";
  let last: Position | null = null;
  let arrivals = 0;
  let lastAccepted: Epoch | null = null;
  const receipts = new Set<Position>();
  const original = new Set<Position>(positions);
  const replayable = new Set<Position>();
  const requests = new Set<Position>();
  const receivedRequests = new Set<Position>();

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "seal": {
          control.port1.postMessage({ last: "Two" });
          const message = receiveMessageOnPort(control.port2);
          assert.ok(message?.message.last === "Two");
          last = message.message.last;
          break;
        }
        case "drop":
          assert.ok(original.delete(command.position));
          break;
        case "request":
          control.port2.postMessage({ retry: command.position });
          requests.add(command.position);
          break;
        case "resend": {
          for (
            let packet = receiveMessageOnPort(control.port1);
            packet;
            packet = receiveMessageOnPort(control.port1)
          ) {
            const { retry } = packet.message;
            assert.ok(retry === "One" || retry === "Two");
            receivedRequests.add(retry);
          }
          assert.ok(receivedRequests.delete(command.position));
          requests.delete(command.position);
          original.add(command.position);
          break;
        }
        case "arrive": {
          const { stream, position } = command;
          if (stream === "Original") {
            assert.ok(original.delete(position) || replayable.has(position));
            replayable.add(position);
          }
          data.port1.postMessage({ stream, position } satisfies Frame);
          const message = receiveMessageOnPort(data.port2);
          assert.ok(message);
          assert.deepEqual(message.message, { stream, position });
          const received = message.message as Frame;
          if (received.stream === epoch || fault === "ignore-epoch") {
            receipts.add(received.position);
            lastAccepted = received.stream;
            arrivals = Math.min(arrivals + 1, positions.length);
          }
          break;
        }
        case "recycle": {
          const needed =
            last === null
              ? []
              : positions.slice(0, positions.indexOf(last) + 1);
          assert.ok(
            last !== null &&
              (fault === "count-only"
                ? arrivals === needed.length
                : needed.every((position) => receipts.has(position))),
            "the sealed range has a gap",
          );
          epoch = "Renewed";
          receipts.clear();
          arrivals = 0;
          lastAccepted = null;
          break;
        }
      }
    },
    async observe() {
      return {
        epoch,
        sealed: last !== null,
        receipts: [...receipts].sort(),
        arrivals,
        lastAccepted,
        waiting: [...original].sort(),
        replayable: [...replayable].sort(),
        requested: [...requests].sort(),
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "a duplicate cannot fill a missing position or count in a later epoch",
  { timeout: 30_000 },
  async (t) => {
    const duplicated = await reachableScenario<Command>(
      checker,
      model,
      "DuplicateMasksGap",
    );
    const outOfOrder = await reachableScenario<Command>(
      checker,
      model,
      "OutOfOrderLeavesGap",
    );
    const repair = (position: Position): readonly Command[] => [
      { kind: "request", position },
      { kind: "resend", position },
      { kind: "arrive", stream: "Original", position },
      { kind: "recycle" },
    ];
    const late: Command = {
      kind: "arrive",
      stream: "Original",
      position: "One",
    };
    const fresh: Command = {
      kind: "arrive",
      stream: "Renewed",
      position: "Two",
    };

    for (const [name, prefix, gap] of [
      ["duplicate masks the second position", duplicated, "Two"],
      ["second position arrives before the first", outOfOrder, "One"],
    ] as const) {
      await t.test(name, async (t) => {
        const scenario = {
          ...prefix,
          commands: [...prefix.commands, ...repair(gap), late, fresh],
        };
        const result = await recordReplay(
          checker,
          scenario,
          openDelivery(t, null),
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-2), {
          epoch: "Renewed",
          sealed: true,
          receipts: [],
          arrivals: 0,
          lastAccepted: null,
          waiting: [],
          replayable: ["One", "Two"],
          requested: [],
        });
        assert.deepEqual(
          (result.recording.observations.at(-1) as { receipts: Position[] })
            .receipts,
          ["Two"],
        );
      });
    }

    await t.test(
      "counting the duplicate releases a ticket with a gap",
      async (t) => {
        const correct = openDelivery(t, null);
        const broken = openDelivery(t, "count-only");
        for (const command of duplicated.commands) {
          await correct.apply(command);
          await broken.apply(command);
        }
        const before = await broken.observe();
        assert.deepEqual(before.receipts, ["One"]);
        assert.equal(before.arrivals, 2);
        await assert.rejects(correct.apply({ kind: "recycle" }), /has a gap/);
        await broken.apply({ kind: "recycle" });
        assert.equal((await broken.observe()).epoch, "Renewed");
      },
    );

    await t.test(
      "ignoring the epoch counts an old copy after safe reuse",
      async (t) => {
        const commands = [...duplicated.commands, ...repair("Two"), late];
        const result = await recordReplay(
          checker,
          { ...duplicated, commands },
          openDelivery(t, "ignore-epoch"),
        );
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, commands.length);
        assert.deepEqual(result.difference.command, late);
        assert.equal(
          (result.difference.expected as { lastAccepted: Epoch | null })
            .lastAccepted,
          null,
        );
        assert.equal(
          (result.difference.observed as { lastAccepted: Epoch | null })
            .lastAccepted,
          "Original",
        );
      },
    );
  },
);
