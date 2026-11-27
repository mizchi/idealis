import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { MessageChannel } from "node:worker_threads";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";

type Candidate = "Primary" | "Hedge";
type Decision =
  | { tag: "Racing" }
  | { tag: "Won" | "Rejected"; value: Candidate };
type Command =
  | { kind: "succeed" | "fail"; candidate: Candidate }
  | { kind: "release" };

class AttemptFailure extends Error {
  constructor(readonly candidate: Candidate) {
    super(`connection from ${candidate} failed`);
  }
}

function attempt(candidate: Candidate) {
  type Result = { candidate: Candidate; channel: MessageChannel };
  let succeed!: (result: Result) => void;
  let fail!: (error: AttemptFailure) => void;
  const promise = new Promise<Result>((resolve, reject) => {
    succeed = resolve;
    fail = reject;
  });
  return {
    promise,
    succeed: (channel: MessageChannel) => succeed({ candidate, channel }),
    fail: () => fail(new AttemptFailure(candidate)),
  };
}

async function close(channel: MessageChannel): Promise<void> {
  const closed = Promise.all([
    once(channel.port1, "close"),
    once(channel.port2, "close"),
  ]);
  channel.port1.close();
  await closed;
}

function openRace(t: TestContext, keepLoser = false) {
  const attempts = { Primary: attempt("Primary"), Hedge: attempt("Hedge") };
  const settled = { Primary: "Pending", Hedge: "Pending" };
  const open = new Map<Candidate, MessageChannel>();
  let decision: Decision = { tag: "Racing" };
  const winner = Promise.race(
    Object.values(attempts).map(({ promise }) => promise),
  ).then(
    ({ candidate, channel }) => {
      decision = { tag: "Won", value: candidate };
      return channel;
    },
    (error: unknown) => {
      if (!(error instanceof AttemptFailure)) throw error;
      decision = { tag: "Rejected", value: error.candidate };
      return undefined;
    },
  );
  t.after(async () => {
    await Promise.all([...open.values()].map(close));
  });
  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "succeed": {
          const { candidate } = command;
          const channel = new MessageChannel();
          open.set(candidate, channel);
          channel.port1.once("close", () => open.delete(candidate));
          settled[candidate] = "Succeeded";
          attempts[candidate].succeed(channel);
          const selected = await winner;
          if (selected !== channel && !keepLoser) await close(channel);
          break;
        }
        case "fail":
          settled[command.candidate] = "Failed";
          attempts[command.candidate].fail();
          await winner;
          break;
        case "release": {
          const channel = await winner;
          assert.ok(channel);
          await close(channel);
          break;
        }
      }
    },
    async observe() {
      return {
        attempts: { ...settled },
        decision: { ...decision },
        open: [...open.keys()].sort(),
      };
    },
    project: (observation: Readonly<Record<string, unknown>>) => observation,
  };
}

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/node-event-loop/race-resource.model";

test(
  "a settled race closes a late resource independently of the returned result",
  { timeout: 15_000 },
  async (t) => {
    for (const [name, loser, expectedOpen, actualOpen] of [
      ["WonWithHedgePending", "Hedge", ["Primary"], ["Hedge", "Primary"]],
      ["RejectedWithPrimaryPending", "Primary", [], ["Primary"]],
      ["ReleasedWithHedgePending", "Hedge", [], ["Hedge"]],
    ] as const) {
      const prefix = await reachableScenario<Command>(checker, model, name, {
        checker: "rprob",
      });
      const scenario = {
        ...prefix,
        commands: [
          ...prefix.commands,
          { kind: "succeed", candidate: loser } as const,
        ],
      };
      await t.test(name, async (t) => {
        const result = await recordReplay(checker, scenario, openRace(t));
        assert.equal(result.verdict, "matched");
        assert.deepEqual(
          (result.recording.observations.at(-1) as { open: Candidate[] }).open,
          expectedOpen,
        );
      });
      await t.test(`${name} / keeps the loser`, async (t) => {
        const driver = openRace(t, true);
        const result = await recordReplay(checker, scenario, driver);
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, scenario.commands.length);
        const expected = result.difference.expected as {
          open: Candidate[];
          decision: Decision;
        };
        const observed = result.difference.observed as {
          open: Candidate[];
          decision: Decision;
        };
        assert.deepEqual(expected.open, expectedOpen);
        assert.deepEqual(observed.open, actualOpen);
        assert.deepEqual(observed.decision, expected.decision);
        const saved = fromReplayItf(
          JSON.parse(JSON.stringify(toReplayItf(result.recording))),
        );
        assert.deepEqual(
          await replayRecording(checker, saved, driver.project),
          {
            verdict: "mismatch",
            difference: result.difference,
          },
        );
      });
    }
  },
);
