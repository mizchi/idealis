import assert from "node:assert/strict";
import test, { type TestContext } from "node:test";
import { MessageChannel, receiveMessageOnPort } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Work = "First" | "Second";
type Command =
  | { kind: "allocate"; work: Work }
  | {
      kind: "cancel" | "send" | "seal" | "readControl" | "readData" | "recycle";
    };

const checker = "crates/rcheck/target/debug/rcheck";
const model =
  "examples/model-patterns/subscriptions/delivery-ticket-watermark.model";

function openPorts(t: TestContext, fault: "control-only" | null) {
  const data = new MessageChannel();
  const control = new MessageChannel();
  t.after(() => {
    data.port1.close();
    data.port2.close();
    control.port1.close();
    control.port2.close();
  });

  let leaseHeld = false;
  let owner: Work | null = null;
  let producer: { work: Work; posted: number } | null = null;
  let consumed = 0;
  let finalCount: number | null = null;
  let waitingSeal: number | null = null;
  const waitingData: Work[] = [];
  const issued = new Set<Work>();
  const delivered: Record<Work, Work | null> = { First: null, Second: null };

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "allocate":
          assert.equal(leaseHeld, false, "the ticket is still held");
          leaseHeld = true;
          owner = command.work;
          producer = { work: command.work, posted: 0 };
          consumed = 0;
          finalCount = null;
          issued.add(command.work);
          break;
        case "cancel":
          owner = null;
          break;
        case "send":
          assert.ok(producer);
          data.port1.postMessage(producer.work);
          waitingData.push(producer.work);
          producer.posted++;
          break;
        case "seal":
          assert.ok(producer);
          assert.equal(waitingSeal, null);
          control.port1.postMessage(producer.posted);
          waitingSeal = producer.posted;
          producer = null;
          break;
        case "readControl": {
          const frame = receiveMessageOnPort(control.port2);
          assert.ok(frame);
          assert.equal(typeof frame.message, "number");
          assert.equal(frame.message, waitingSeal);
          finalCount = frame.message;
          waitingSeal = null;
          break;
        }
        case "readData": {
          const frame = receiveMessageOnPort(data.port2);
          assert.ok(frame);
          assert.equal(frame.message, waitingData.shift());
          assert.ok(frame.message === "First" || frame.message === "Second");
          consumed++;
          if (owner !== null) delivered[owner] = frame.message;
          break;
        }
        case "recycle":
          assert.ok(
            finalCount !== null &&
              (fault === "control-only" || finalCount === consumed),
            "the control marker and its data must be received",
          );
          leaseHeld = false;
          break;
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
        sender: producer?.work ?? null,
        seen: consumed,
        finalCount,
        control: waitingSeal,
        queued: { $sequence: waitingData.slice() },
        delivered: { ...delivered },
      };
    },
    project: ({
      phase,
      owner,
      sender,
      seen,
      finalCount,
      control,
      queued,
      delivered,
    }: Readonly<Record<string, unknown>>) => ({
      phase,
      owner,
      sender,
      seen,
      finalCount,
      control,
      queued,
      delivered,
    }),
  };
}

test(
  "a control marker cannot release a ticket with outstanding data on another port",
  { timeout: 30_000 },
  async (t) => {
    const controlFirst = await reachableScenario<Command>(
      checker,
      model,
      "ControlOvertakesData",
    );
    const dataFirst = await reachableScenario<Command>(
      checker,
      model,
      "DataOvertakesControl",
    );
    const empty = await reachableScenario<Command>(
      checker,
      model,
      "ZeroDataSealed",
    );
    const reuse: readonly Command[] = [
      { kind: "recycle" },
      { kind: "allocate", work: "Second" },
      { kind: "send" },
      { kind: "readData" },
    ];

    for (const scenario of [
      {
        name: "control first",
        prefix: controlFirst,
        drain: [{ kind: "readData" }, { kind: "readData" }],
      },
      {
        name: "data first",
        prefix: dataFirst,
        drain: [{ kind: "readControl" }],
      },
      { name: "zero is a received watermark", prefix: empty, drain: [] },
    ] as const) {
      await t.test(scenario.name, async (t) => {
        const result = await recordReplay(
          checker,
          {
            ...scenario.prefix,
            commands: [
              ...scenario.prefix.commands,
              ...scenario.drain,
              ...reuse,
            ],
          },
          openPorts(t, null),
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          phase: "Active",
          owner: "Second",
          sender: "Second",
          seen: 1,
          finalCount: null,
          control: null,
          queued: { $sequence: [] },
          delivered: { First: null, Second: "Second" },
        });
      });
    }

    await t.test(
      "a partial data drain cannot release the ticket",
      async (t) => {
        const adapter = openPorts(t, null);
        for (const command of controlFirst.commands)
          await adapter.apply(command);
        await assert.rejects(
          adapter.apply({ kind: "recycle" }),
          /must be received/,
        );
        await adapter.apply({ kind: "readData" });
        await assert.rejects(
          adapter.apply({ kind: "recycle" }),
          /must be received/,
        );
        await adapter.apply({ kind: "readData" });
        await adapter.apply({ kind: "recycle" });
        assert.equal((await adapter.observe()).phase, "Available");
      },
    );

    await t.test(
      "an empty data port still needs the control marker",
      async (t) => {
        const adapter = openPorts(t, null);
        for (const command of dataFirst.commands) await adapter.apply(command);
        await assert.rejects(
          adapter.apply({ kind: "recycle" }),
          /must be received/,
        );
        await adapter.apply({ kind: "readControl" });
        await adapter.apply({ kind: "recycle" });
        assert.equal((await adapter.observe()).phase, "Available");
      },
    );

    await t.test(
      "a control-only release delivers an old frame to the new owner",
      async (t) => {
        const adapter = openPorts(t, "control-only");
        for (const command of controlFirst.commands)
          await adapter.apply(command);
        await adapter.apply({ kind: "recycle" });
        await adapter.apply({ kind: "allocate", work: "Second" });
        await adapter.apply({ kind: "readData" });
        const snapshot = await adapter.observe();
        assert.equal(snapshot.delivered.Second, "First");
        assert.deepEqual(snapshot.queued, { $sequence: ["First"] });
      },
    );
  },
);
