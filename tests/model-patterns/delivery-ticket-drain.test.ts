import assert from "node:assert/strict";
import test, { type TestContext } from "node:test";
import { MessageChannel, receiveMessageOnPort } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Work = "First" | "Second";
type Frame = { tag: "Notice"; value: Work } | { tag: "Seal" };
type Command =
  | { kind: "allocate"; work: Work }
  | { kind: "cancel" | "send" | "seal" | "receive" };
type Fault = "release-on-send" | "release-when-empty" | null;

const checker = "crates/rcheck/target/debug/rcheck";
const model =
  "examples/model-patterns/subscriptions/delivery-ticket-drain.model";

function openTicketPort(t: TestContext, fault: Fault) {
  const { port1, port2 } = new MessageChannel();
  t.after(() => {
    port1.close();
    port2.close();
  });
  let leaseHeld = false;
  let owner: Work | null = null;
  let sender: Work | null = null;
  const issued = new Set<Work>();
  const pending: Frame[] = [];
  const delivered: Record<Work, Work | null> = { First: null, Second: null };

  function post(frame: Frame) {
    port1.postMessage(frame);
    pending.push(frame);
  }

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "allocate":
          assert.equal(leaseHeld, false, "ticket is still held");
          issued.add(command.work);
          leaseHeld = true;
          owner = sender = command.work;
          break;
        case "cancel":
          owner = null;
          if (fault === "release-when-empty" && pending.length === 0) {
            leaseHeld = false;
          }
          break;
        case "send":
          assert.ok(sender);
          post({ tag: "Notice", value: sender });
          break;
        case "seal":
          sender = null;
          post({ tag: "Seal" });
          if (fault === "release-on-send") leaseHeld = false;
          break;
        case "receive": {
          const item = receiveMessageOnPort(port2);
          assert.ok(item, "no physical message to read");
          const expected = pending.shift();
          assert.deepEqual(item.message, expected);
          const frame = item.message as Frame;
          if (frame.tag === "Seal") leaseHeld = false;
          else if (owner !== null) delivered[owner] = frame.value;
          break;
        }
      }
    },
    async observe() {
      return {
        phase: leaseHeld
          ? owner === null
            ? "Retiring"
            : "Active"
          : issued.size === 0
            ? "Fresh"
            : "Available",
        owner,
        sender,
        issued: [...issued].sort(),
        queued: { $sequence: pending.slice() },
        delivered: { ...delivered },
      };
    },
    project: ({
      phase,
      owner,
      sender,
      issued,
      queued,
      delivered,
    }: Readonly<Record<string, unknown>>) => ({
      phase,
      owner,
      sender,
      issued,
      queued,
      delivered,
    }),
  };
}

test(
  "a ticket becomes reusable only after receiving the producer's seal in FIFO order",
  { timeout: 30_000 },
  async (t) => {
    const sealed = await reachableScenario<Command>(
      checker,
      model,
      "SealQueuedWhileRetiring",
      { checker: "rprob" },
    );
    const empty = await reachableScenario<Command>(
      checker,
      model,
      "CancelledBeforeSend",
      { checker: "rprob" },
    );
    const drainAndReuse: Command[] = [
      { kind: "receive" },
      { kind: "receive" },
      { kind: "allocate", work: "Second" },
      { kind: "send" },
      { kind: "receive" },
    ];
    for (const scenario of [
      {
        name: "queued notice survives sender shutdown",
        prefix: sealed,
        tail: drainAndReuse,
        fault: null,
      },
      {
        name: "empty queue can still receive an old notice",
        prefix: empty,
        tail: [{ kind: "send" }, { kind: "seal" }, ...drainAndReuse],
        fault: null,
      },
      {
        name: "sender posting the seal releases too early",
        prefix: sealed,
        tail: [],
        fault: "release-on-send",
      },
      {
        name: "looking at the empty mailbox releases too early",
        prefix: empty,
        tail: [],
        fault: "release-when-empty",
      },
    ] as const) {
      await t.test(scenario.name, async (t) => {
        const adapter = openTicketPort(t, scenario.fault);
        const actual = await recordReplay(
          checker,
          {
            ...scenario.prefix,
            commands: [...scenario.prefix.commands, ...scenario.tail],
          },
          adapter,
        );
        if (scenario.fault === null) {
          assert.equal(actual.verdict, "matched");
          assert.deepEqual(actual.recording.observations.at(-1), {
            phase: "Active",
            owner: "Second",
            sender: "Second",
            issued: ["First", "Second"],
            queued: { $sequence: [] },
            delivered: { First: null, Second: "Second" },
          });
        } else {
          assert.equal(actual.verdict, "mismatch");
          assert.equal(actual.difference.step, scenario.prefix.commands.length);
          assert.deepEqual(
            actual.difference.command,
            scenario.prefix.commands.at(-1),
          );
          assert.equal(
            (actual.difference.expected as { phase: string }).phase,
            "Retiring",
          );
          assert.equal(
            (actual.difference.observed as { phase: string }).phase,
            "Available",
          );
        }
      });
    }

    await t.test(
      "releasing on send delivers the old notice to the new owner",
      async (t) => {
        const correct = openTicketPort(t, null);
        const broken = openTicketPort(t, "release-on-send");
        for (const command of sealed.commands) {
          await correct.apply(command);
          await broken.apply(command);
        }
        const reuse: Command = { kind: "allocate", work: "Second" };
        await assert.rejects(correct.apply(reuse), /ticket is still held/);
        await broken.apply(reuse);
        await broken.apply({ kind: "receive" });
        assert.equal((await broken.observe()).delivered.Second, "First");
      },
    );
  },
);
