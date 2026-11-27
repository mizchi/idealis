import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";

type Worker = "East" | "West";
type Checkpoint = { pending: { $sequence: string[] }; retries: number };
type Command = { kind: "checkpoint" | "expire"; worker: Worker };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/worker-checkpoints.model";
const project = (state: Readonly<Record<string, unknown>>) => state;

const checkpoints: Record<Worker, Checkpoint> = {
  East: { pending: { $sequence: ["First", "Second"] }, retries: 1 },
  West: { pending: { $sequence: ["Fourth", "Third"] }, retries: 2 },
};

test("expiring a SQLite checkpoint leaves the other worker's checkpoint intact", async () => {
  const witness = await reachableScenario<Command>(
    checker,
    model,
    "BothSaved",
    {
      checker: "rprob",
    },
  );
  assert.deepEqual(witness.commands, [
    { kind: "checkpoint", worker: "East" },
    { kind: "checkpoint", worker: "West" },
  ]);
  const scenario = {
    ...witness,
    commands: [
      ...witness.commands,
      { kind: "expire", worker: "East" } as const,
    ],
  };
  for (const clearAll of [false, true]) {
    using database = new DatabaseSync(":memory:");
    database.exec(
      "CREATE TABLE saved (worker TEXT PRIMARY KEY, queue TEXT NOT NULL, retries INTEGER NOT NULL)",
    );
    const save = database.prepare("INSERT INTO saved VALUES (?, ?, ?)");
    const read = database.prepare(
      "SELECT queue, retries FROM saved WHERE worker = ?",
    );
    const remove = database.prepare(
      clearAll
        ? "DELETE FROM saved WHERE ? IS NOT NULL"
        : "DELETE FROM saved WHERE worker = ?",
    );
    const checkpoint = (worker: Worker): Checkpoint | null => {
      const row = read.get(worker);
      return row === undefined
        ? null
        : {
            pending: { $sequence: JSON.parse(String(row.queue)) as string[] },
            retries: Number(row.retries),
          };
    };
    const result = await recordReplay(checker, scenario, {
      async apply(command) {
        if (command.kind === "expire") {
          remove.run(command.worker);
        } else {
          const { pending, retries } = checkpoints[command.worker];
          save.run(command.worker, JSON.stringify(pending.$sequence), retries);
        }
      },
      async observe() {
        return {
          saved: { East: checkpoint("East"), West: checkpoint("West") },
        };
      },
      project,
    });
    if (!clearAll) {
      assert.equal(result.verdict, "matched");
      assert.deepEqual(result.recording.observations.at(-1), {
        saved: { East: null, West: checkpoints.West },
      });
      continue;
    }
    assert.equal(result.verdict, "mismatch");
    assert.equal(result.difference.step, 3);
    assert.deepEqual(result.difference.observed, {
      saved: { East: null, West: null },
    });
    const saved = fromReplayItf(
      JSON.parse(JSON.stringify(toReplayItf(result.recording))),
    );
    assert.deepEqual(await replayRecording(checker, saved, project), {
      verdict: "mismatch",
      difference: result.difference,
    });
  }
});
