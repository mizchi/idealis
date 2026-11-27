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

type Command = { kind: "retry" };
type State = { pending: { $sequence: string[] } };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/rotate-queue.model";

test("retry moves the SQLite queue head to the tail, preserving duplicate work", async () => {
  const scenario = await reachableScenario<Command>(checker, model, "Retried", {
    checker: "rprob",
  });
  assert.deepEqual(scenario.commands, [{ kind: "retry" }]);
  const starting = (scenario.initial as State).pending.$sequence;
  for (const keepHead of [false, true]) {
    using database = new DatabaseSync(":memory:");
    database.exec(
      "CREATE TABLE pending (position INTEGER PRIMARY KEY AUTOINCREMENT, work TEXT NOT NULL)",
    );
    const insert = database.prepare("INSERT INTO pending (work) VALUES (?)");
    for (const work of starting) insert.run(work);
    const result = await recordReplay(checker, scenario, {
      async apply() {
        database.exec("BEGIN");
        database.exec(
          "INSERT INTO pending (work) SELECT work FROM pending ORDER BY position LIMIT 1",
        );
        if (!keepHead) {
          database.exec(
            "DELETE FROM pending WHERE position = (SELECT MIN(position) FROM pending)",
          );
        }
        database.exec("COMMIT");
      },
      async observe(): Promise<State> {
        const rows = database
          .prepare("SELECT work FROM pending ORDER BY position")
          .all();
        return { pending: { $sequence: rows.map((row) => String(row.work)) } };
      },
      project: (state) => state,
    });
    if (!keepHead) {
      assert.equal(result.verdict, "matched");
      assert.deepEqual(result.recording.observations[1], {
        pending: { $sequence: ["Second", "Third", "Second", "First"] },
      });
      continue;
    }
    assert.equal(result.verdict, "mismatch");
    assert.equal(result.difference.step, 1);
    assert.deepEqual(result.difference.observed, {
      pending: { $sequence: ["First", "Second", "Third", "Second", "First"] },
    });
    const itf = JSON.parse(JSON.stringify(toReplayItf(result.recording)));
    const replay = await replayRecording(
      checker,
      fromReplayItf(itf),
      (state) => state,
    );
    assert.deepEqual(replay, {
      verdict: "mismatch",
      difference: result.difference,
    });
  }
});
