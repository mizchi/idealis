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

type Registration = "Old" | "New";
type Sequence = "First" | "Second";
type Command =
  | { kind: "rotate" | "seal" | "collect" | "reopen" }
  | { kind: "submit"; registration: Registration; sequence: Sequence };
type Fault = "delete-without-fence" | "sequence-fence" | "seal-only";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/receipt-collection.model";
const oldRetry = {
  kind: "submit",
  registration: "Old",
  sequence: "First",
} as const;
const newFirst = {
  kind: "submit",
  registration: "New",
  sequence: "First",
} as const;

async function openReceiver(t: TestContext, fault?: Fault) {
  const root = await mkdtemp(join(tmpdir(), "idealis-receipt-collection-"));
  const path = join(root, "transport.sqlite");
  let db = new DatabaseSync(path);
  t.after(async () => {
    db.close();
    await rm(root, { recursive: true, force: true });
  });
  db.exec(`
    CREATE TABLE progress (active TEXT NOT NULL, sealed INTEGER NOT NULL, closed INTEGER NOT NULL);
    INSERT INTO progress VALUES ('Old', 0, 0);
    CREATE TABLE receipts (registration TEXT, sequence TEXT, PRIMARY KEY(registration, sequence));
    CREATE TABLE effects (registration TEXT, sequence TEXT);
  `);

  let reply: "Applied" | "Duplicate" | "Expired" | null = null;
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
  function state() {
    const progress = db.prepare("SELECT * FROM progress").get();
    assert.ok(progress);
    return progress;
  }
  function rows(table: "effects" | "receipts", registration: Registration) {
    return db
      .prepare(`SELECT DISTINCT sequence FROM ${table}
      WHERE registration = ? ORDER BY sequence`)
      .all(registration)
      .map((row) => row.sequence);
  }

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "rotate":
          db.exec("UPDATE progress SET active = 'New'");
          break;
        case "seal":
          db.exec("UPDATE progress SET sealed = 1");
          break;
        case "reopen":
          db.close();
          db = new DatabaseSync(path);
          break;
        case "collect":
          transact(() => {
            const { active, sealed, closed } = state();
            if (active !== "New" || sealed !== 1 || closed === 1) return;
            if (fault !== "seal-only" && rows("receipts", "Old").length !== 2)
              return;
            if (fault !== "delete-without-fence")
              db.exec("UPDATE progress SET closed = 1");
            db.exec("DELETE FROM receipts WHERE registration = 'Old'");
          });
          break;
        case "submit":
          transact(() => {
            const { registration, sequence } = command;
            if (
              state().closed === 1 &&
              (registration === "Old" || fault === "sequence-fence")
            ) {
              reply = "Expired";
            } else if (
              db
                .prepare(
                  "SELECT 1 FROM receipts WHERE registration = ? AND sequence = ?",
                )
                .get(registration, sequence)
            ) {
              reply = "Duplicate";
            } else {
              db.prepare("INSERT INTO effects VALUES (?, ?)").run(
                registration,
                sequence,
              );
              db.prepare("INSERT INTO receipts VALUES (?, ?)").run(
                registration,
                sequence,
              );
              reply = "Applied";
            }
          });
          break;
      }
    },
    async observe() {
      const progress = state();
      return {
        active: progress.active,
        sealed: progress.sealed === 1,
        closed: progress.closed === 1,
        receipts: {
          Old: rows("receipts", "Old"),
          New: rows("receipts", "New"),
        },
        applied: { Old: rows("effects", "Old"), New: rows("effects", "New") },
        effects: db.prepare("SELECT COUNT(*) AS count FROM effects").get()
          ?.count,
        reply,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "collecting old receipts keeps stale deliveries fenced after reopening",
  { timeout: 30_000 },
  async (t) => {
    const collected = await transitionScenario<Command>(
      checker,
      model,
      "collect",
      "after(closed) and Second in before(New.receipts) and no before(First & New.receipts)",
    );
    const complete = {
      ...collected,
      commands: [
        ...collected.commands,
        { kind: "reopen" } as const,
        oldRetry,
        newFirst,
      ],
    };
    await t.test(
      "the old retry is expired and the new sequence is still admissible",
      async (t) => {
        const run = await recordReplay(
          checker,
          complete,
          await openReceiver(t),
        );
        assert.equal(run.verdict, "matched");
        const old = run.recording.observations.at(-2) as {
          reply: string;
          effects: number;
        };
        assert.deepEqual([old.reply, old.effects], ["Expired", 3]);
        assert.deepEqual(run.recording.observations.at(-1), {
          active: "New",
          sealed: true,
          closed: true,
          effects: 4,
          reply: "Applied",
          receipts: { Old: [], New: ["First", "Second"] },
          applied: { Old: ["First", "Second"], New: ["First", "Second"] },
        });
      },
    );
    await t.test(
      "deleting receipts without persisting the fence reapplies a stale frame",
      async (t) => {
        const adapter = await openReceiver(t, "delete-without-fence");
        const run = await recordReplay(checker, complete, adapter);
        assert.equal(run.verdict, "mismatch");
        assert.deepEqual(run.difference.command, { kind: "collect" });
        for (const command of complete.commands.slice(run.difference.step, -1))
          await adapter.apply(command);
        const state = await adapter.observe();
        assert.equal(state.effects, 4);
        assert.equal(state.reply, "Applied");
      },
    );
    await t.test(
      "a fence shared by sequence suppresses a new registration",
      async (t) => {
        const run = await recordReplay(
          checker,
          complete,
          await openReceiver(t, "sequence-fence"),
        );
        assert.equal(run.verdict, "mismatch");
        assert.deepEqual(run.difference.command, newFirst);
        assert.equal(
          (run.difference.observed as { reply: string }).reply,
          "Expired",
        );
      },
    );
    await t.test(
      "a seal alone cannot close a registration with a missing frame",
      async (t) => {
        const beforeDrain = await transitionScenario<Command>(
          checker,
          model,
          "collect",
          "before(Old.receipts) = First and unchanged(closed)",
        );
        const missing = {
          kind: "submit",
          registration: "Old",
          sequence: "Second",
        } as const;
        const normal = await recordReplay(
          checker,
          {
            ...beforeDrain,
            commands: [...beforeDrain.commands, missing, { kind: "collect" }],
          },
          await openReceiver(t),
        );
        assert.equal(normal.verdict, "matched");
        assert.equal(
          (normal.recording.observations.at(-1) as { closed: boolean }).closed,
          true,
        );
        const adapter = await openReceiver(t, "seal-only");
        const faulty = await recordReplay(checker, beforeDrain, adapter);
        assert.equal(faulty.verdict, "mismatch");
        assert.deepEqual(faulty.difference.command, { kind: "collect" });
        await adapter.apply(missing);
        const observation = await adapter.observe();
        assert.equal(observation.reply, "Expired");
        assert.deepEqual(observation.applied.Old, ["First"]);
      },
    );
  },
);
