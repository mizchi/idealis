import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { MessageChannel } from "node:worker_threads";
import {
  transitionScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const attempts = ["First", "Second", "Third"] as const;
type Attempt = (typeof attempts)[number];
type Checkpoint = "Older" | "Newer";
type Stage = "Recovering" | "RestartRequired" | "Ready" | "Published";
type Command =
  | { kind: "request" | "expire" | "switchCheckpoint" | "retry" | "publish" }
  | { kind: "receive"; attempt: Attempt };
type Row = { readonly key: string; readonly checkpoint: Checkpoint };
type Response = { readonly attempt: Attempt; readonly row: Row };
type Fault = "by-row" | "by-checkpoint";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/partition-restart.model";

function openRecovery(t: TestContext, fault?: Fault) {
  const snapshots: Record<Checkpoint, Row> = {
    Older: { key: "order-42", checkpoint: "Older" },
    Newer: { key: "order-42", checkpoint: "Newer" },
  };
  const requested = new Map<Attempt, { row: Row; channel: MessageChannel }>();
  const responses = new Map<Attempt, Response>();
  const expired = new Set<Checkpoint>();
  let epoch = 0;
  let checkpoint: Checkpoint = "Older";
  let stage: Stage = "Recovering";
  let cached: Response | undefined;
  let exposed: Row | undefined;

  function activeAttempt() {
    const attempt = attempts[epoch];
    assert.ok(attempt);
    return attempt;
  }

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "request": {
          assert.ok(!expired.has(checkpoint));
          const attempt = activeAttempt();
          const channel = new MessageChannel();
          t.after(() => {
            channel.port1.close();
            channel.port2.close();
          });
          const row = snapshots[checkpoint];
          requested.set(attempt, { row, channel });
          // The producer has read the row. Delivery can lag on this channel.
          channel.port1.postMessage({ attempt, row });
          break;
        }
        case "expire":
          expired.add(checkpoint);
          stage = "RestartRequired";
          break;
        case "switchCheckpoint":
          checkpoint = "Newer";
          epoch++;
          stage = "Recovering";
          break;
        case "retry":
          epoch++;
          break;
        case "receive": {
          const request = requested.get(command.attempt);
          assert.ok(request);
          const [response] = (await once(request.channel.port2, "message")) as [
            Response,
          ];
          responses.set(response.attempt, response);
          const current =
            fault === "by-row"
              ? response.row.key === snapshots[checkpoint].key
              : fault === "by-checkpoint"
                ? response.row.checkpoint === checkpoint
                : response.attempt === activeAttempt();
          if (stage === "Recovering" && current) {
            cached = response;
            stage = "Ready";
          }
          break;
        }
        case "publish":
          assert.ok(cached);
          exposed = cached.row;
          stage = "Published";
          break;
      }
    },
    async observe() {
      return {
        active: activeAttempt(),
        checkpoint,
        requested: Object.fromEntries(
          attempts.map((attempt) => [
            attempt,
            requested.get(attempt)?.row.checkpoint ?? null,
          ]),
        ),
        delivered: [...responses.keys()].sort(),
        expired: [...expired].sort(),
        accepted: cached?.attempt ?? null,
        visible: exposed?.checkpoint ?? null,
        stage,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "restart isolates in-flight replies by recovery attempt",
  { timeout: 30_000 },
  async (t) => {
    for (const { target, obsolete, current, fault } of [
      {
        target: "OlderReplyInFlight",
        obsolete: "First",
        current: "Second",
        fault: "by-row",
      },
      {
        target: "SameCheckpointReplyInFlight",
        obsolete: "Second",
        current: "Third",
        fault: "by-checkpoint",
      },
    ] as const) {
      const scenario = await transitionScenario<Command>(
        checker,
        model,
        "receive",
        `attempt = ${obsolete} and before(active) = ${current} and
         before(stage) = Recovering and no before(delivered) and
         some before(${current}.requested) and unchanged(accepted)`,
      );
      const delayed: Command = { kind: "receive", attempt: obsolete };
      assert.deepEqual(scenario.commands.at(-1), delayed);
      await t.test(target, async (t) => {
        const result = await recordReplay(
          checker,
          {
            ...scenario,
            commands: [
              ...scenario.commands,
              { kind: "receive", attempt: current },
              { kind: "publish" },
            ],
          },
          openRecovery(t),
        );
        assert.equal(result.verdict, "matched");
        const afterObsolete = result.recording.observations[
          scenario.commands.length
        ] as {
          delivered: Attempt[];
          stage: Stage;
          accepted: Attempt | null;
          visible: Checkpoint | null;
        };
        assert.ok(afterObsolete.delivered.includes(obsolete));
        assert.equal(afterObsolete.stage, "Recovering");
        assert.equal(afterObsolete.accepted, null);
        assert.equal(afterObsolete.visible, null);
        assert.deepEqual(result.recording.observations.at(-1), {
          ...afterObsolete,
          active: current,
          checkpoint: "Newer",
          requested: {
            First: "Older",
            Second: "Newer",
            Third: current === "Third" ? "Newer" : null,
          },
          delivered: [...afterObsolete.delivered, current].sort(),
          expired: ["Older"],
          accepted: current,
          visible: "Newer",
          stage: "Published",
        });
      });
      await t.test(`${target} after publication`, async (t) => {
        const result = await recordReplay(
          checker,
          {
            ...scenario,
            commands: [
              ...scenario.commands.slice(0, -1),
              { kind: "receive", attempt: current },
              { kind: "publish" },
              delayed,
            ],
          },
          openRecovery(t),
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          ...(result.recording.observations.at(-2) as Record<string, unknown>),
          delivered: [current, obsolete].sort(),
        });
      });
      await t.test(fault, async (t) => {
        const result = await recordReplay(
          checker,
          scenario,
          openRecovery(t, fault),
        );
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, scenario.commands.length);
        assert.equal(
          (result.difference.observed as { accepted: Attempt }).accepted,
          obsolete,
        );
      });
    }
  },
);
