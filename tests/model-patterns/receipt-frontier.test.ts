import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import {
  recordReplay,
  transitionScenario,
} from "../../examples/model-patterns/replay.js";

type Generation = "G1" | "G2" | "G3";
type Sequence = "One" | "Two";
type Command =
  | { kind: "seal" | "collect"; generation: Generation }
  | { kind: "compact" | "reopen" }
  | { kind: "submit"; generation: Generation; sequence: Sequence };
type Fault = "greatest-frontier" | "ignore-sparse";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/receipt-frontier.model";
const generations = ["Zero", "G1", "G2", "G3"] as const;
const firstMissing = {
  kind: "submit",
  generation: "G1",
  sequence: "Two",
} as const;
const secondRetry = {
  kind: "submit",
  generation: "G2",
  sequence: "One",
} as const;

async function receiver(t: TestContext, fault?: Fault) {
  const root = await mkdtemp(join(tmpdir(), "idealis-receipt-frontier-"));
  const path = join(root, "receipts.sqlite");
  let db = new DatabaseSync(path);
  t.after(async () => {
    db.close();
    await rm(root, { recursive: true, force: true });
  });
  db.exec(`
    CREATE TABLE boundary (generation INTEGER NOT NULL);
    INSERT INTO boundary VALUES (0);
    CREATE TABLE sealed (generation INTEGER PRIMARY KEY);
    CREATE TABLE sparse (generation INTEGER PRIMARY KEY);
    CREATE TABLE receipts (generation INTEGER, sequence TEXT, PRIMARY KEY(generation, sequence));
    CREATE TABLE effects (generation INTEGER, sequence TEXT);
  `);

  let reply: "Applied" | "Duplicate" | "Expired" | null = null;
  function frontier() {
    return Number(
      db.prepare("SELECT generation FROM boundary").get()?.generation,
    );
  }
  function earlyClosed(generation: number) {
    return (
      db
        .prepare("SELECT 1 FROM sparse WHERE generation = ?")
        .get(generation) !== undefined
    );
  }
  function generationRows(sql: string) {
    return db
      .prepare(sql)
      .all()
      .map((row) => generations[Number(row.generation)]);
  }
  function partitionRows(sql: string) {
    const select = db.prepare(sql);
    return Object.fromEntries(
      generations.map((name, generation) => [
        name,
        select.all(generation).map((row) => row.sequence),
      ]),
    );
  }
  function transact(work: () => void) {
    db.exec("BEGIN IMMEDIATE");
    try {
      work();
      db.exec("COMMIT");
    } catch (error) {
      db.exec("ROLLBACK");
      throw error;
    }
  }

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "reopen":
          db.close();
          db = new DatabaseSync(path);
          return;
        case "compact":
          transact(() => {
            const next = frontier() + 1;
            const chosen =
              fault === "greatest-frontier"
                ? db
                    .prepare("SELECT max(generation) AS generation FROM sparse")
                    .get()?.generation
                : earlyClosed(next)
                  ? next
                  : undefined;
            if (typeof chosen !== "number") return;
            db.prepare("UPDATE boundary SET generation = ?").run(chosen);
            db.prepare("DELETE FROM sparse WHERE generation <= ?").run(chosen);
          });
          return;
        case "seal":
          db.prepare("INSERT INTO sealed VALUES (?)").run(
            generations.indexOf(command.generation),
          );
          return;
        case "collect":
          transact(() => {
            const generation = generations.indexOf(command.generation);
            const existing = db
              .prepare(
                "SELECT COUNT(*) AS count FROM receipts WHERE generation = ?",
              )
              .get(generation)?.count;
            if (existing !== 2) return;
            db.prepare("INSERT INTO sparse VALUES (?)").run(generation);
            db.prepare("DELETE FROM sealed WHERE generation = ?").run(
              generation,
            );
            db.prepare("DELETE FROM receipts WHERE generation = ?").run(
              generation,
            );
          });
          return;
        case "submit":
          transact(() => {
            const generation = generations.indexOf(command.generation);
            if (
              generation <= frontier() ||
              (fault !== "ignore-sparse" && earlyClosed(generation))
            ) {
              reply = "Expired";
            } else if (
              db
                .prepare(
                  "SELECT 1 FROM receipts WHERE generation = ? AND sequence = ?",
                )
                .get(generation, command.sequence)
            ) {
              reply = "Duplicate";
            } else {
              db.prepare("INSERT INTO receipts VALUES (?, ?)").run(
                generation,
                command.sequence,
              );
              db.prepare("INSERT INTO effects VALUES (?, ?)").run(
                generation,
                command.sequence,
              );
              reply = "Applied";
            }
          });
          return;
      }
    },
    async observe() {
      return {
        frontier: generations[frontier()],
        sealed: generationRows(
          "SELECT generation FROM sealed ORDER BY generation",
        ),
        earlyClosed: generationRows(
          "SELECT generation FROM sparse ORDER BY generation",
        ),
        receipts: partitionRows(
          "SELECT sequence FROM receipts WHERE generation = ? ORDER BY sequence",
        ),
        applied: partitionRows(
          "SELECT DISTINCT sequence FROM effects WHERE generation = ? ORDER BY sequence",
        ),
        effects: db.prepare("SELECT COUNT(*) AS count FROM effects").get()
          ?.count,
        reply,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test("receipt compaction waits for the gap and fences deleted receipts after reopening", async (t) => {
  const gap = await transitionScenario<Command>(
    checker,
    model,
    "compact",
    "before(frontier) = Zero and before(earlyClosed) = G2 and before(G1.applied) = One and G1 not in before(sealed) and unchanged(frontier)",
  );
  assert.equal(gap.commands.length, 6);
  const afterGap = {
    ...gap,
    commands: [
      ...gap.commands,
      { kind: "reopen" },
      secondRetry,
    ] satisfies Command[],
  };
  await t.test("closing the gap compresses both generations", async (t) => {
    const run = await recordReplay(
      checker,
      {
        ...gap,
        commands: [
          ...afterGap.commands,
          firstMissing,
          { kind: "seal", generation: "G1" },
          { kind: "collect", generation: "G1" },
          { kind: "compact" },
          { kind: "compact" },
          { kind: "reopen" },
          firstMissing,
          secondRetry,
          { kind: "submit", generation: "G3", sequence: "One" },
        ],
      },
      await receiver(t),
    );
    assert.equal(run.verdict, "matched");
    const observations = run.recording.observations as {
      frontier: string;
      earlyClosed: string[];
      sealed: string[];
      effects: number;
      reply: string | null;
    }[];
    assert.deepEqual(
      observations
        .slice(gap.commands.length + 1)
        .map(({ frontier, earlyClosed, reply }) => [
          frontier,
          earlyClosed,
          reply,
        ]),
      [
        ["Zero", ["G2"], "Applied"],
        ["Zero", ["G2"], "Expired"],
        ["Zero", ["G2"], "Applied"],
        ["Zero", ["G2"], "Applied"],
        ["Zero", ["G1", "G2"], "Applied"],
        ["G1", ["G2"], "Applied"],
        ["G2", [], "Applied"],
        ["G2", [], "Applied"],
        ["G2", [], "Expired"],
        ["G2", [], "Expired"],
        ["G2", [], "Applied"],
      ],
    );
    assert.equal(observations.at(-1)?.effects, 5);
    assert.deepEqual(observations.at(-1)?.sealed, []);
  });
  await t.test(
    "taking the greatest closed generation loses a missing older frame",
    async (t) => {
      const adapter = await receiver(t, "greatest-frontier");
      const run = await recordReplay(checker, gap, adapter);
      assert.equal(run.verdict, "mismatch");
      assert.equal(run.difference.step, 6);
      assert.deepEqual(run.difference.command, { kind: "compact" });
      await adapter.apply({ kind: "reopen" });
      await adapter.apply(firstMissing);
      const observed = await adapter.observe();
      assert.equal(observed.reply, "Expired");
      assert.deepEqual(observed.applied.G1, ["One"]);
    },
  );
  await t.test(
    "forgetting the sparse fence applies a duplicate after reopening",
    async (t) => {
      const run = await recordReplay(
        checker,
        afterGap,
        await receiver(t, "ignore-sparse"),
      );
      assert.equal(run.verdict, "mismatch");
      assert.equal(run.difference.step, 8);
      assert.deepEqual(run.difference.command, secondRetry);
      const actual = run.difference.observed as {
        effects: number;
        reply: string;
      };
      assert.equal(actual.reply, "Applied");
      assert.equal(actual.effects, 4);
    },
  );
});
