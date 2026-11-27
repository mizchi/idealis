import assert from "node:assert/strict";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import { MessageChannel } from "node:worker_threads";
import {
  recordReplay,
  transitionScenario,
} from "../../examples/model-patterns/replay.js";

type Attempt = "First" | "Second";
type Part = "Boundary" | "Ledger";
type Revision = "Target" | "Older";
type Reply = {
  readonly attempt: Attempt;
  readonly part: Part;
  readonly revision: Revision;
};
type Command =
  | { kind: "begin" | "abort" | "publish" | "reopen" }
  | { kind: "request"; part: Part }
  | ({ kind: "receive" } & Reply);
type Fault = "relabel-old" | "clear-current-on-old";
const checker = "crates/rcheck/target/debug/rcheck";
const model =
  "examples/model-patterns/crash-recovery/receipt-restore-attempt.model";
const oldLedger = {
  kind: "receive",
  attempt: "First",
  part: "Ledger",
  revision: "Target",
} as const;
const currentLedger = { ...oldLedger, attempt: "Second" } as const;
const publish = { kind: "publish" } as const;

async function openRestore(t: TestContext, fault?: Fault) {
  const root = await mkdtemp(join(tmpdir(), "idealis-restore-attempt-"));
  const path = join(root, "restore.sqlite");
  let db = new DatabaseSync(path);
  const { port1, port2 } = new MessageChannel();
  t.after(async () => {
    port1.close();
    port2.close();
    db.close();
    await rm(root, { recursive: true, force: true });
  });
  db.exec(`
    CREATE TABLE control (phase TEXT NOT NULL, attempt TEXT NOT NULL);
    INSERT INTO control VALUES ('Idle', 'First');
    CREATE TABLE sent (attempt TEXT, part TEXT, PRIMARY KEY (attempt, part));
    CREATE TABLE received (attempt TEXT, part TEXT, PRIMARY KEY (attempt, part));
    CREATE TABLE staging (part TEXT PRIMARY KEY, attempt TEXT NOT NULL, revision TEXT NOT NULL);
  `);
  const control = () => {
    const row = db.prepare("SELECT * FROM control").get();
    assert.ok(row);
    return row;
  };
  const stages = (part: Part) => {
    const row = db
      .prepare("SELECT attempt, revision FROM staging WHERE part = ?")
      .get(part);
    return row ? { ...row } : null;
  };
  const links = (sql: string) => {
    const match = db.prepare(sql);
    return {
      First: match.all("First").map((row) => row.part),
      Second: match.all("Second").map((row) => row.part),
    };
  };
  const transact = (body: () => void) => {
    db.exec("BEGIN IMMEDIATE");
    try {
      body();
      db.exec("COMMIT");
    } catch (error) {
      db.exec("ROLLBACK");
      throw error;
    }
  };
  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "reopen":
          db.close();
          db = new DatabaseSync(path);
          return;
        case "begin":
          db.exec(`UPDATE control
            SET attempt = CASE phase WHEN 'Aborted' THEN 'Second' ELSE attempt END,
            phase = 'Fetching'`);
          return;
        case "abort":
          transact(() =>
            db.exec(
              "UPDATE control SET phase = 'Aborted'; DELETE FROM staging",
            ),
          );
          return;
        case "request":
          db.prepare("INSERT INTO sent SELECT attempt, ? FROM control").run(
            command.part,
          );
          return;
        case "receive": {
          const incoming = once(port1, "message");
          port2.postMessage({
            attempt: command.attempt,
            part: command.part,
            revision: command.revision,
          });
          const [reply] = (await incoming) as [Reply];
          transact(() => {
            db.prepare("INSERT INTO received VALUES (?, ?)").run(
              reply.attempt,
              reply.part,
            );
            const current = control();
            if (current.phase !== "Fetching" || reply.revision !== "Target")
              return;
            if (reply.attempt !== current.attempt) {
              if (fault === "clear-current-on-old")
                db.prepare("DELETE FROM staging WHERE part = ?").run(
                  reply.part,
                );
              if (fault !== "relabel-old") return;
            }
            db.prepare("INSERT OR REPLACE INTO staging VALUES (?, ?, ?)").run(
              reply.part,
              String(current.attempt),
              reply.revision,
            );
          });
          return;
        }
        case "publish":
          db.exec(`UPDATE control SET phase = 'Published' WHERE phase = 'Fetching'
            AND (SELECT COUNT(*) FROM staging WHERE attempt = control.attempt AND revision = 'Target') = 2`);
          return;
      }
    },
    async observe() {
      const current = control();
      return {
        phase: current.phase,
        active: current.attempt,
        sent: links("SELECT part FROM sent WHERE attempt = ? ORDER BY part"),
        received: links(
          "SELECT part FROM received WHERE attempt = ? ORDER BY part",
        ),
        staged: { Boundary: stages("Boundary"), Ledger: stages("Ledger") },
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test("same-revision delayed pieces cannot replace or complete the new restore", async (t) => {
  const straddled = await transitionScenario<Command>(
    checker,
    model,
    "receive",
    [
      "attempt = First",
      "part = Boundary",
      "revision = Target",
      "before(active) = Second",
      "before(phase) = Fetching",
      "before(Boundary.staged) = Offer { attempt: Second, revision: Target }",
      "before(First.sent) = Part",
      "before(Second.sent) = Part",
      "no before(Ledger.staged)",
      "no before(First.received)",
    ].join(" and "),
  );
  assert.equal(straddled.commands.length, 9);
  const [lateBoundary] = straddled.commands.slice(-1);
  assert.ok(lateBoundary);
  const lateBoundaryPath = {
    ...straddled,
    commands: [
      ...straddled.commands.slice(0, -1),
      { kind: "reopen" },
      lateBoundary,
    ] satisfies Command[],
  };
  const delayed = {
    ...lateBoundaryPath,
    commands: [
      ...lateBoundaryPath.commands,
      oldLedger,
      publish,
      currentLedger,
      publish,
    ],
  };
  await t.test(
    "both late pieces are inert and current pieces can finish",
    async (t) => {
      const run = await recordReplay(checker, delayed, await openRestore(t));
      assert.equal(run.verdict, "matched", JSON.stringify(run));
      const before = run.recording.observations.at(-3) as {
        phase: string;
        staged: { Ledger: unknown };
      };
      assert.equal(before.phase, "Fetching");
      assert.equal(before.staged.Ledger, null);
      assert.deepEqual(run.recording.observations.at(-1), {
        phase: "Published",
        active: "Second",
        sent: { First: ["Boundary", "Ledger"], Second: ["Boundary", "Ledger"] },
        received: {
          First: ["Boundary", "Ledger"],
          Second: ["Boundary", "Ledger"],
        },
        staged: {
          Boundary: { attempt: "Second", revision: "Target" },
          Ledger: { attempt: "Second", revision: "Target" },
        },
      });
    },
  );
  await t.test(
    "aborting clears an already staged ledger without losing the outstanding request",
    async (t) => {
      const aborted = await transitionScenario<Command>(
        checker,
        model,
        "abort",
        "before(Ledger.staged) = Offer { attempt: First, revision: Target } and before(First.sent) = Part and Boundary not in before(First.received) and no after(staged)",
      );
      const run = await recordReplay(
        checker,
        {
          ...aborted,
          commands: [
            ...aborted.commands,
            { kind: "reopen" },
            { kind: "begin" },
            {
              kind: "receive",
              attempt: "First",
              part: "Boundary",
              revision: "Target",
            },
            publish,
          ],
        },
        await openRestore(t),
      );
      assert.equal(run.verdict, "matched");
      assert.deepEqual(run.recording.observations.at(-1), {
        phase: "Fetching",
        active: "Second",
        sent: { First: ["Boundary", "Ledger"], Second: [] },
        received: { First: ["Boundary", "Ledger"], Second: [] },
        staged: { Boundary: null, Ledger: null },
      });
    },
  );
  await t.test(
    "the current attempt still rejects a piece with the wrong revision",
    async (t) => {
      const run = await recordReplay(
        checker,
        {
          ...lateBoundaryPath,
          commands: [
            ...lateBoundaryPath.commands,
            { ...currentLedger, revision: "Older" },
            publish,
          ],
        },
        await openRestore(t),
      );
      assert.equal(run.verdict, "matched");
      const observed = run.recording.observations.at(-1) as {
        phase: string;
        staged: { Ledger: unknown };
        received: { Second: Part[] };
      };
      assert.equal(observed.phase, "Fetching");
      assert.equal(observed.staged.Ledger, null);
      assert.deepEqual(observed.received.Second, ["Boundary", "Ledger"]);
    },
  );
  await t.test(
    "relabeling the old ledger allows premature publication",
    async (t) => {
      const adapter = await openRestore(t, "relabel-old");
      const run = await recordReplay(checker, delayed, adapter);
      assert.equal(run.verdict, "mismatch");
      assert.equal(run.difference.step, 11);
      assert.deepEqual(run.difference.command, oldLedger);
      await adapter.apply(publish);
      const observed = await adapter.observe();
      assert.equal(observed.phase, "Published");
      assert.deepEqual(observed.received.Second, ["Boundary"]);
    },
  );
  await t.test(
    "an old boundary must not delete the current boundary",
    async (t) => {
      const adapter = await openRestore(t, "clear-current-on-old");
      const run = await recordReplay(checker, delayed, adapter);
      assert.equal(run.verdict, "mismatch");
      assert.equal(run.difference.step, 10);
      assert.deepEqual(run.difference.command, lateBoundary);
      await adapter.apply(currentLedger);
      await adapter.apply(publish);
      const observed = await adapter.observe();
      assert.equal(observed.phase, "Fetching");
      assert.equal(observed.staged.Boundary, null);
    },
  );
});
