import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
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

type Checkpoint = {
  pending: { $sequence: string[] };
  retries: number;
};
type State = { saved: Checkpoint; restored: Checkpoint | null };
type Command = { kind: "restart" };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/restored-queue.model";

test("SQLite recovery preserves the queue order and retry counter from the discovered checkpoint", async (t) => {
  const scenario = await reachableScenario<Command>(
    checker,
    model,
    "RestoresFullQueue",
    { checker: "rprob" },
  );
  assert.deepEqual(scenario.commands, [{ kind: "restart" }]);

  const root = await mkdtemp(join(tmpdir(), "idealis-restored-queue-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const path = join(root, "queue.sqlite");
  const { pending, retries } = (scenario.initial as State).saved;
  {
    using writer = new DatabaseSync(path);
    writer.exec(`
      CREATE TABLE queued (position INTEGER PRIMARY KEY, work TEXT NOT NULL);
      CREATE TABLE checkpoint (retries INTEGER NOT NULL);
    `);
    writer.prepare("INSERT INTO checkpoint VALUES (?)").run(retries);
    const insert = writer.prepare("INSERT INTO queued VALUES (?, ?)");
    for (const [position, work] of pending.$sequence.entries()) {
      insert.run(position, work);
    }
  }

  const recover = (order: "position" | "work"): Checkpoint => {
    using reader = new DatabaseSync(path);
    const sql =
      order === "position"
        ? "SELECT work FROM queued ORDER BY position"
        : "SELECT work FROM queued ORDER BY work";
    const queued = reader
      .prepare(sql)
      .all()
      .map((row) => String(row.work));
    return {
      pending: { $sequence: queued },
      retries: Number(
        reader.prepare("SELECT retries FROM checkpoint").get()?.retries,
      ),
    };
  };
  for (const faulty of [false, true]) {
    let restored: Checkpoint | null = null;
    const result = await recordReplay(checker, scenario, {
      async apply() {
        restored = recover(faulty ? "work" : "position");
      },
      async observe(): Promise<State> {
        return { saved: recover("position"), restored };
      },
      project: (state) => state,
    });
    if (!faulty) {
      assert.equal(result.verdict, "matched");
      continue;
    }
    assert.equal(result.verdict, "mismatch");
    assert.equal(result.difference.step, 1);
    const expected = result.difference.expected as State;
    const observed = result.difference.observed as State;
    assert.deepEqual(observed.saved, expected.saved);
    assert.equal(observed.restored?.retries, expected.restored?.retries);
    assert.deepEqual(expected.restored?.pending.$sequence, [
      "Fourth",
      "Third",
      "Second",
      "First",
    ]);
    assert.deepEqual(observed.restored?.pending.$sequence, [
      "First",
      "Fourth",
      "Second",
      "Third",
    ]);
  }
});

test("SQLite restart copies the stored queue and consumes one available retry", async () => {
  const scenario = await reachableScenario<Command>(
    checker,
    "examples/model-patterns/crash-recovery/restart-budget.model",
    "RestartsFullQueue",
    { checker: "rprob" },
  );
  const { saved } = scenario.initial as State;
  assert.equal(saved.retries, 7);
  assert.deepEqual(scenario.commands, [{ kind: "restart" }]);

  for (const debit of [1, 0]) {
    using database = new DatabaseSync(":memory:");
    database.exec(`
      CREATE TABLE saved (retries INTEGER);
      CREATE TABLE restored (retries INTEGER);
      CREATE TABLE queue (position INTEGER PRIMARY KEY, work TEXT);
      CREATE TABLE recovered (position INTEGER PRIMARY KEY, work TEXT);
    `);
    database.prepare("INSERT INTO saved VALUES (?)").run(saved.retries);
    const insert = database.prepare("INSERT INTO queue VALUES (?, ?)");
    for (const [position, work] of saved.pending.$sequence.entries()) {
      insert.run(position, work);
    }
    const restart = database.prepare(
      "INSERT INTO restored SELECT retries - ? FROM saved WHERE retries > 0",
    );
    const readSaved = database.prepare("SELECT retries FROM saved");
    const readRestored = database.prepare("SELECT retries FROM restored");
    const readQueue = database.prepare(
      "SELECT work FROM queue ORDER BY position",
    );
    const readRecovered = database.prepare(
      "SELECT work FROM recovered ORDER BY position",
    );
    const pending = (rows: Record<string, unknown>[]) => ({
      $sequence: rows.map((row) => String(row.work)),
    });
    const result = await recordReplay(checker, scenario, {
      async apply() {
        database.exec("BEGIN");
        if (restart.run(debit).changes !== 0) {
          database.exec(
            "INSERT INTO recovered SELECT position, work FROM queue",
          );
        }
        database.exec("COMMIT");
      },
      async observe(): Promise<State> {
        const restored = readRestored.get();
        return {
          saved: {
            retries: Number(readSaved.get()?.retries),
            pending: pending(readQueue.all()),
          },
          restored:
            restored === undefined
              ? null
              : {
                  retries: Number(restored.retries),
                  pending: pending(readRecovered.all()),
                },
        };
      },
      project: (state) => state,
    });
    if (debit === 1) {
      assert.equal(result.verdict, "matched");
      continue;
    }
    assert.equal(result.verdict, "mismatch");
    assert.equal(result.difference.step, 1);
    assert.deepEqual((result.difference.expected as State).restored, {
      ...saved,
      retries: 6,
    });
    assert.deepEqual((result.difference.observed as State).restored, saved);
    const itf = JSON.parse(JSON.stringify(toReplayItf(result.recording)));
    assert.deepEqual(
      await replayRecording(checker, fromReplayItf(itf), (state) => state),
      { verdict: "mismatch", difference: result.difference },
    );
  }
});
