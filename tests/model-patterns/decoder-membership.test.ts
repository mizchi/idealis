import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { MessageChannel } from "node:worker_threads";
import {
  recordReplay,
  transitionScenario,
} from "../../examples/model-patterns/replay.js";

type LegacyRegistration = "Original" | "Secondary";
type Registration = LegacyRegistration | "Replacement";
type Phase = "Idle" | "Draining" | "Retired";
type Command =
  | { kind: "begin" | "replace" | "retire" | "takeReplacement" }
  | {
      kind: "sendLegacy" | "takeLegacy";
      registration: LegacyRegistration;
    }
  | {
      kind: "stop" | "takeSeal";
      registration: Registration;
    };
type Fault = "follow-live" | "discard-detached";
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/crash-recovery/decoder-membership.ideal";

function openReceiver(t: TestContext, fault?: Fault) {
  function channel() {
    const ports = new MessageChannel();
    t.after(() => {
      ports.port1.close();
      ports.port2.close();
    });
    return ports;
  }
  function streams() {
    return { control: channel(), legacy: channel() };
  }
  const streamsByRegistration = {
    Original: streams(),
    Secondary: streams(),
    Replacement: { control: channel() },
  };
  const replacement = channel();
  let live = { Primary: "Original", Hedge: "Secondary" } as Record<
    "Primary" | "Hedge",
    Registration
  >;
  let phase: Phase = "Idle";
  let cohort: Registration[] = [];
  const stopped = new Set<Registration>();
  const sent = new Set<Registration>();
  const taken = new Set<Registration>();
  const seals = new Map<Registration, number>();
  const decoded = new Set<Registration>();
  const dropped = new Set<Registration>();
  let replacementDecoded = false;

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "begin":
          cohort = Object.values(live);
          phase = "Draining";
          break;
        case "replace":
          live = { ...live, Primary: "Replacement" };
          replacement.port1.postMessage(Uint8Array.of(2, 0));
          break;
        case "retire": {
          const waitFor =
            fault === "follow-live" ? Object.values(live) : cohort;
          if (
            waitFor.every((registration) => {
              const promised = seals.get(registration);
              return (
                promised === 0 || (promised === 1 && decoded.has(registration))
              );
            })
          )
            phase = "Retired";
          break;
        }
        case "takeReplacement": {
          const [frame] = await once(replacement.port2, "message");
          assert.deepEqual(frame, Uint8Array.of(2, 0));
          replacementDecoded = true;
          break;
        }
        case "sendLegacy":
          assert.ok(!stopped.has(command.registration));
          sent.add(command.registration);
          streamsByRegistration[command.registration].legacy.port1.postMessage(
            Uint8Array.of(1, 1),
          );
          break;
        case "stop":
          stopped.add(command.registration);
          streamsByRegistration[command.registration].control.port1.postMessage(
            Number(sent.has(command.registration)),
          );
          break;
        case "takeSeal": {
          const [count] = await once(
            streamsByRegistration[command.registration].control.port2,
            "message",
          );
          assert.ok(count === 0 || count === 1);
          taken.add(command.registration);
          if (
            fault !== "discard-detached" ||
            Object.values(live).includes(command.registration)
          ) {
            seals.set(command.registration, count);
          }
          break;
        }
        case "takeLegacy": {
          const [frame] = await once(
            streamsByRegistration[command.registration].legacy.port2,
            "message",
          );
          assert.deepEqual(frame, Uint8Array.of(1, 1));
          (phase === "Retired" ? dropped : decoded).add(command.registration);
          break;
        }
      }
    },
    async observe() {
      return {
        live,
        phase,
        cohort: [...cohort].sort(),
        stopped: [...stopped].sort(),
        sent: [...sent].sort(),
        taken: [...taken].sort(),
        sealed: [...seals.keys()].sort(),
        promised: [...seals]
          .filter(([, count]) => count === 1)
          .map(([registration]) => registration)
          .sort(),
        decoded: [...decoded].sort(),
        dropped: [...dropped].sort(),
        replacementDecoded,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "decoder retirement waits for captured registrations across a slot replacement",
  { timeout: 30_000 },
  async (t) => {
    const setup = await transitionScenario<Command>(
      checker,
      model,
      "retire",
      "before(Primary.live) = Replacement and before(sealed) = Replacement + Secondary and before(sent) = Original and Original not in before(stopped) and no before(decoded) and unchanged(phase)",
    );
    const oldSeal = { kind: "takeSeal", registration: "Original" } as const;
    const oldData = { kind: "takeLegacy", registration: "Original" } as const;
    const finish: Command[] = [
      { kind: "stop", registration: "Original" },
      oldSeal,
      oldData,
      { kind: "retire" },
      { kind: "takeReplacement" },
    ];
    const complete = { ...setup, commands: [...setup.commands, ...finish] };

    await t.test(
      "the detached sender still belongs to the drain",
      async (t) => {
        const run = await recordReplay(checker, complete, openReceiver(t));
        assert.equal(run.verdict, "matched");
        const state = run.recording.observations.at(-1) as {
          phase: Phase;
          cohort: Registration[];
          decoded: Registration[];
          replacementDecoded: boolean;
        };
        assert.equal(state.phase, "Retired");
        assert.deepEqual(state.cohort, ["Original", "Secondary"]);
        assert.deepEqual(state.decoded, ["Original"]);
        assert.equal(state.replacementDecoded, true);
      },
    );
    await t.test("the replacement need not send a legacy seal", async (t) => {
      const scenario = await transitionScenario<Command>(
        checker,
        model,
        "retire",
        `before(Primary.live) = Replacement and Replacement not in before(taken) and
         Original in before(decoded) and after(phase) = Retired`,
      );
      const run = await recordReplay(checker, scenario, openReceiver(t));
      assert.equal(run.verdict, "matched");
      const state = run.recording.observations.at(-1) as {
        phase: Phase;
        sealed: Registration[];
      };
      assert.equal(state.phase, "Retired");
      assert.deepEqual(state.sealed, ["Original", "Secondary"]);
    });
    await t.test(
      "following live membership drops the detached sender's old frame",
      async (t) => {
        const adapter = openReceiver(t, "follow-live");
        const run = await recordReplay(checker, setup, adapter);
        assert.equal(run.verdict, "mismatch");
        assert.equal(run.difference.step, setup.commands.length);
        await adapter.apply(oldData);
        assert.deepEqual((await adapter.observe()).dropped, ["Original"]);
      },
    );
    await t.test(
      "discarding a detached sender's seal prevents retirement",
      async (t) => {
        const adapter = openReceiver(t, "discard-detached");
        const run = await recordReplay(checker, complete, adapter);
        assert.equal(run.verdict, "mismatch");
        assert.deepEqual(run.difference.command, oldSeal);
        for (const command of complete.commands.slice(run.difference.step)) {
          await adapter.apply(command);
        }
        const state = await adapter.observe();
        assert.deepEqual(state.decoded, ["Original"]);
        assert.equal(state.phase, "Draining");
        assert.equal(state.replacementDecoded, true);
      },
    );
  },
);
