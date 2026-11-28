import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = { kind: "advanceFloor" | "collect" };
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/offline-merge/batch-retention.ideal";

test("batch expiry removes both older receipts and retains the cutoff row", async (t) => {
  const scenario = await reachableScenario<Command>(
    checker,
    model,
    "RemovedBothAndKeptBoundary",
  );
  assert.deepEqual(
    scenario.commands.map(({ kind }) => kind),
    ["advanceFloor", "collect"],
  );
  const normal =
    "DELETE FROM receipts WHERE generation < (SELECT floor FROM retention)";
  const expected = {
    Earliest: null,
    Earlier: null,
    Boundary: 3,
    Future: 4,
    Unseen: null,
  };
  for (const [name, statement, observed] of [
    ["exact cutoff", normal, expected],
    [
      "deletes the boundary",
      normal.replace("generation <", "generation <="),
      { ...expected, Boundary: null },
    ],
    [
      "deletes only the oldest",
      "DELETE FROM receipts WHERE generation = (SELECT MIN(generation) FROM receipts WHERE generation < (SELECT floor FROM retention))",
      { ...expected, Earlier: 2 },
    ],
  ] as const) {
    await t.test(name, async () => {
      using database = new DatabaseSync(":memory:");
      database.exec(`
        CREATE TABLE retention (floor INTEGER NOT NULL);
        INSERT INTO retention VALUES (0);
        CREATE TABLE receipts (ticket TEXT PRIMARY KEY, generation INTEGER, revision INTEGER);
        INSERT INTO receipts VALUES ('Earliest', 0, 1), ('Earlier', 1, 2), ('Boundary', 2, 3), ('Future', 3, 4);
      `);
      const result = await recordReplay(checker, scenario, {
        async apply({ kind }) {
          database.exec(
            kind === "advanceFloor"
              ? "UPDATE retention SET floor = 2"
              : statement,
          );
        },
        async observe() {
          const rows = database
            .prepare("SELECT ticket, revision FROM receipts")
            .all();
          const receipts = {
            Earliest: null,
            Earlier: null,
            Boundary: null,
            Future: null,
            Unseen: null,
            ...Object.fromEntries(
              rows.map(({ ticket, revision }) => [ticket, revision]),
            ),
          };
          return {
            floor: database.prepare("SELECT floor FROM retention").get()?.floor,
            receipts,
          };
        },
        project: (state) => state,
      });
      if (statement === normal) {
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          floor: 2,
          receipts: expected,
        });
      } else {
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, 2);
        assert.deepEqual(result.difference.expected, {
          floor: 2,
          receipts: expected,
        });
        assert.deepEqual(result.difference.observed, {
          floor: 2,
          receipts: observed,
        });
      }
    });
  }
});
