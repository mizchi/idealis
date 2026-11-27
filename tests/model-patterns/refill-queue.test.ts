import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Batch = { $sequence: string[] };
type History = { $sequence: Batch[] };
type Command = { kind: "refill"; history: History };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/refill-queue.model";

test("the discovered refill argument preserves batch order in SQLite", async () => {
  const scenario = await reachableScenario<Command>(
    checker,
    model,
    "Refilled",
    {
      checker: "rprob",
    },
  );
  assert.deepEqual(scenario.commands, [
    {
      kind: "refill",
      history: {
        $sequence: [{ $sequence: ["Third"] }, { $sequence: ["Fourth"] }],
      },
    },
  ]);

  for (const reverse of [false, true]) {
    using database = new DatabaseSync(":memory:");
    database.exec(
      "CREATE TABLE batch (position INTEGER PRIMARY KEY, items TEXT NOT NULL)",
    );
    const insert = database.prepare("INSERT INTO batch VALUES (?, ?)");
    const select = database.prepare(
      reverse
        ? "SELECT items FROM batch ORDER BY position DESC"
        : "SELECT items FROM batch ORDER BY position",
    );
    const run = await recordReplay(checker, scenario, {
      async apply(command) {
        for (const [position, batch] of command.history.$sequence.entries()) {
          insert.run(position, JSON.stringify(batch.$sequence));
        }
      },
      async observe() {
        const rows = select.all();
        return {
          pending:
            rows.length === 0
              ? null
              : {
                  $sequence: rows.map((row) => ({
                    $sequence: JSON.parse(String(row.items)) as string[],
                  })),
                },
        };
      },
      project: (state) => state,
    });
    if (!reverse) {
      assert.equal(run.verdict, "matched");
      continue;
    }
    assert.equal(run.verdict, "mismatch");
    assert.equal(run.difference.step, 1);
    assert.deepEqual(run.difference.observed, {
      pending: {
        $sequence: [{ $sequence: ["Fourth"] }, { $sequence: ["Third"] }],
      },
    });
  }
});
