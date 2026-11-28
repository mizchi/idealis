import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { MessageChannel } from "node:worker_threads";
import {
  recordReplay,
  transitionScenario,
} from "../../examples/model-patterns/replay.js";

type Producer = "Alpha" | "Beta";
type Command =
  | { kind: "retire" }
  | {
      kind:
        | "sendLegacy"
        | "stop"
        | "takeSeal"
        | "takeLegacy"
        | "sendCurrent"
        | "takeCurrent";
      producer: Producer;
    };
type Fault = "first-seal" | "seals-only" | "count-current";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/decoder-retirement.model";
const producers = ["Alpha", "Beta"] as const;

function openReceiver(t: TestContext, fault?: Fault) {
  function channel() {
    const ports = new MessageChannel();
    t.after(() => {
      ports.port1.close();
      ports.port2.close();
    });
    return ports;
  }
  function channels() {
    return { legacy: channel(), current: channel(), control: channel() };
  }
  const ports = { Alpha: channels(), Beta: channels() };
  const stopped = new Set<Producer>();
  const sentLegacy = new Set<Producer>();
  const sentCurrent = new Set<Producer>();
  const seals = new Map<Producer, number>();
  const decodedLegacy = new Set<Producer>();
  const decodedCurrent = new Set<Producer>();
  const droppedLegacy = new Set<Producer>();
  const legacyDecoder = (bytes: Uint8Array) => bytes[1] === 1;
  let decode: typeof legacyDecoder | undefined = legacyDecoder;

  return {
    async apply(command: Command) {
      if (command.kind === "retire") {
        const advertisedReads = [...seals.values()].reduce(
          (sum, count) => sum + count,
          0,
        );
        const drained =
          fault === "count-current"
            ? [...seals].every(
                ([sender, count]) =>
                  count === 0 ||
                  decodedLegacy.has(sender) ||
                  decodedCurrent.has(sender),
              )
            : decodedLegacy.size === advertisedReads;
        if (
          (fault === "first-seal" && seals.size > 0) ||
          (seals.size === producers.length &&
            (fault === "seals-only" || drained))
        ) {
          decode = undefined;
        }
        return;
      }
      const { producer } = command;
      const { legacy, current, control } = ports[producer];
      switch (command.kind) {
        case "sendLegacy":
          assert.ok(!stopped.has(producer));
          legacy.port1.postMessage(Uint8Array.of(1, 1));
          sentLegacy.add(producer);
          break;
        case "sendCurrent":
          assert.ok(stopped.has(producer));
          current.port1.postMessage(Uint8Array.of(2, 0));
          sentCurrent.add(producer);
          break;
        case "stop":
          stopped.add(producer);
          control.port1.postMessage(Number(sentLegacy.has(producer)));
          break;
        case "takeSeal": {
          const [count] = await once(control.port2, "message");
          assert.ok(count === 0 || count === 1);
          seals.set(producer, count);
          break;
        }
        case "takeLegacy": {
          const [bytes] = (await once(legacy.port2, "message")) as [Uint8Array];
          assert.equal(bytes[0], 1);
          if (decode?.(bytes)) decodedLegacy.add(producer);
          else droppedLegacy.add(producer);
          break;
        }
        case "takeCurrent": {
          const [bytes] = (await once(current.port2, "message")) as [
            Uint8Array,
          ];
          assert.deepEqual(bytes, Uint8Array.of(2, 0));
          decodedCurrent.add(producer);
          break;
        }
      }
    },
    async observe() {
      return {
        decoder: decode ? "Both" : "CurrentOnly",
        stopped: [...stopped].sort(),
        sentLegacy: [...sentLegacy].sort(),
        sealed: [...seals.keys()].sort(),
        promised: producers.filter((producer) => seals.get(producer) === 1),
        decodedLegacy: [...decodedLegacy].sort(),
        droppedLegacy: [...droppedLegacy].sort(),
        sentCurrent: [...sentCurrent].sort(),
        decodedCurrent: [...decodedCurrent].sort(),
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "retirement requires every sender's seal and every promised legacy reply",
  { timeout: 30_000 },
  async (t) => {
    for (const [fault, predicate] of [
      [
        "first-seal",
        "before(sealed) = Alpha and before(stopped) = Alpha and before(sentLegacy) = Beta",
      ],
      ["seals-only", "before(sealed) = Producer and before(promised) = Beta"],
      [
        "count-current",
        "before(sealed) = Producer and before(promised) = Beta and before(decodedCurrent) = Beta",
      ],
    ] as const) {
      const scenario = await transitionScenario<Command>(
        checker,
        model,
        "retire",
        `(${predicate}) and no before(decodedLegacy) and unchanged(decoder)`,
      );
      await t.test(`normal after ${fault} setup`, async (t) => {
        const extra: Command[] = [{ kind: "takeLegacy", producer: "Beta" }];
        if (fault === "first-seal") {
          extra.push(
            { kind: "stop", producer: "Beta" },
            { kind: "takeSeal", producer: "Beta" },
          );
        }
        extra.push({ kind: "retire" });
        if (fault !== "count-current") {
          extra.push(
            { kind: "sendCurrent", producer: "Beta" },
            { kind: "takeCurrent", producer: "Beta" },
          );
        }
        const run = await recordReplay(
          checker,
          { ...scenario, commands: [...scenario.commands, ...extra] },
          openReceiver(t),
        );
        assert.equal(run.verdict, "matched");
        const final = run.recording.observations.at(-1) as {
          decoder: string;
          decodedLegacy: Producer[];
          decodedCurrent: Producer[];
          droppedLegacy: Producer[];
        };
        assert.equal(final.decoder, "CurrentOnly");
        assert.deepEqual(final.decodedLegacy, ["Beta"]);
        assert.deepEqual(final.decodedCurrent, ["Beta"]);
        assert.deepEqual(final.droppedLegacy, []);
      });
      await t.test(fault, async (t) => {
        const adapter = openReceiver(t, fault);
        const run = await recordReplay(checker, scenario, adapter);
        assert.equal(run.verdict, "mismatch");
        assert.equal(run.difference.step, scenario.commands.length);
        await adapter.apply({ kind: "takeLegacy", producer: "Beta" });
        assert.deepEqual((await adapter.observe()).droppedLegacy, ["Beta"]);
      });
    }

    await t.test("zero legacy replies still allow retirement", async (t) => {
      const scenario = await transitionScenario<Command>(
        checker,
        model,
        "retire",
        "no before(sentLegacy) and after(decoder) = CurrentOnly",
      );
      const run = await recordReplay(checker, scenario, openReceiver(t));
      assert.equal(run.verdict, "matched");
      assert.equal(
        (run.recording.observations.at(-1) as { decoder: string }).decoder,
        "CurrentOnly",
      );
    });
  },
);
