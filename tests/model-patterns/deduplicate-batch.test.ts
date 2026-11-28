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

type Batch = { $sequence: string[] };
type State = { applied: { $sequence: Batch[] } };
type Command = { kind: "receive"; batch: Batch };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/deduplicate-batch.model";
const project = (state: Readonly<Record<string, unknown>>) => state;

test("a SQLite batch journal deduplicates by complete ordered content", async () => {
  const witness = await reachableScenario<Command>(
    checker,
    model,
    "AppliedBoth",
    {
      checker: "rprob",
    },
  );
  assert.deepEqual(witness.commands, [
    { kind: "receive", batch: { $sequence: ["First", "Third"] } },
  ]);
  const [novel] = witness.commands;
  assert.ok(novel);
  const initial = (witness.initial as State).applied.$sequence;
  const [seen] = initial;
  assert.ok(seen);
  const scenario = {
    ...witness,
    commands: [
      { kind: "receive", batch: seen } as const,
      ...witness.commands,
      ...witness.commands,
    ],
  };

  for (const keyByHead of [false, true]) {
    using database = new DatabaseSync(":memory:");
    database.exec(`
      CREATE TABLE applied (
        position INTEGER PRIMARY KEY,
        fingerprint TEXT NOT NULL UNIQUE,
        batch TEXT NOT NULL
      )
    `);
    const insert = database.prepare(`
      INSERT OR IGNORE INTO applied (fingerprint, batch)
      SELECT ?, ? WHERE (SELECT COUNT(*) FROM applied) < 2
    `);
    const write = (batch: Batch) => {
      const body = JSON.stringify(batch.$sequence);
      insert.run(keyByHead ? JSON.stringify(batch.$sequence[0]) : body, body);
    };
    for (const batch of initial) write(batch);
    const read = database.prepare(
      "SELECT batch FROM applied ORDER BY position",
    );
    const result = await recordReplay(checker, scenario, {
      async apply(command) {
        write(command.batch);
      },
      async observe(): Promise<State> {
        return {
          applied: {
            $sequence: read.all().map((row) => ({
              $sequence: JSON.parse(String(row.batch)) as string[],
            })),
          },
        };
      },
      project,
    });
    if (!keyByHead) {
      assert.equal(result.verdict, "matched");
      assert.deepEqual(result.recording.observations.at(-1), {
        applied: { $sequence: [seen, novel.batch] },
      });
      continue;
    }
    assert.equal(result.verdict, "mismatch");
    assert.equal(result.difference.step, 2);
    assert.deepEqual(result.difference.observed, {
      applied: { $sequence: [seen] },
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
