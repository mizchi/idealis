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

type Revision = "BeforeClose" | "AfterClose";
type Frame = "OldOne" | "OldTwo" | "NewOne" | "NewTwo";
type Command =
  | { kind: "beginRestore" | "install" | "reopen" | "abortRestore" }
  | { kind: "receiveFence" | "receiveLedger"; revision: Revision }
  | { kind: "submit"; frame: Frame };
type Fault =
  | "fence-only"
  | "ledger-only"
  | "ignore-local-effects"
  | "mixed-revisions"
  | "abort-unverified"
  | "abort-copies-staged-receipts";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/receipt-checkpoint.model";
const oldRetry = { kind: "submit", frame: "OldTwo" } as const;
const newRetry = { kind: "submit", frame: "NewOne" } as const;
const fresh = { kind: "submit", frame: "NewTwo" } as const;
const latestFence = { kind: "receiveFence", revision: "AfterClose" } as const;
const latestLedger = { kind: "receiveLedger", revision: "AfterClose" } as const;
const install = { kind: "install" } as const;
const reopen = { kind: "reopen" } as const;
const abortRestore = { kind: "abortRestore" } as const;

async function openReplicas(t: TestContext, fault?: Fault) {
  const dir = await mkdtemp(join(tmpdir(), "idealis-receipt-checkpoint-"));
  let source: DatabaseSync | null = new DatabaseSync(
    join(dir, "source.sqlite"),
  );
  const targetPath = join(dir, "target.sqlite");
  let target = new DatabaseSync(targetPath);
  t.after(async () => {
    source?.close();
    target.close();
    await rm(dir, { recursive: true, force: true });
  });
  source.exec(`
    CREATE TABLE checkpoint (revision TEXT PRIMARY KEY, closed INTEGER, applied TEXT, receipts TEXT);
    INSERT INTO checkpoint VALUES
      ('BeforeClose', 0, '["OldOne"]', '["OldOne"]'),
      ('AfterClose', 1, '["OldOne","OldTwo","NewOne"]', '["NewOne"]');
  `);
  target.exec(`
    CREATE TABLE control (ready INTEGER, closed INTEGER, required TEXT, local_revision TEXT);
    INSERT INTO control VALUES (0, 0, 'AfterClose', NULL);
    CREATE TABLE verified_effects (frame TEXT PRIMARY KEY);
    CREATE TABLE fence_offer (revision TEXT, closed INTEGER);
    CREATE TABLE ledger_offer (revision TEXT, applied TEXT, receipts TEXT);
    CREATE TABLE effects (frame TEXT NOT NULL);
    INSERT INTO effects VALUES ('OldOne');
    CREATE TABLE receipts (frame TEXT PRIMARY KEY);
    INSERT INTO receipts VALUES ('OldOne');
  `);
  let reply: "Applied" | "Expired" | "Duplicate" | null = null;
  const control = () => {
    const row = target.prepare("SELECT * FROM control").get();
    assert.ok(row);
    return row;
  };
  const applied = () =>
    target
      .prepare("SELECT DISTINCT frame FROM effects ORDER BY frame")
      .all()
      .map((row) => String(row.frame));
  const checkpoint = (revision: Revision) => {
    assert.ok(source);
    const row = source
      .prepare("SELECT * FROM checkpoint WHERE revision = ?")
      .get(revision);
    assert.ok(row);
    return row;
  };
  const hasVerifiedLocalImage = () => {
    const current = control();
    if (current.local_revision !== current.required || current.closed !== 1)
      return false;
    const proof = target
      .prepare(`
      SELECT NOT EXISTS (
        SELECT frame FROM verified_effects EXCEPT SELECT frame FROM effects
      ) AS covers_verified_effects,
      NOT EXISTS (
        SELECT frame FROM effects WHERE frame NOT IN ('OldOne', 'OldTwo')
        EXCEPT SELECT frame FROM receipts
      ) AND NOT EXISTS (
        SELECT frame FROM receipts
        EXCEPT SELECT frame FROM effects WHERE frame NOT IN ('OldOne', 'OldTwo')
      ) AS receipts_match,
      (SELECT COUNT(*) FROM effects) = (SELECT COUNT(DISTINCT frame) FROM effects) AS unique_effects
    `)
      .get();
    return (
      proof?.covers_verified_effects === 1 &&
      proof.receipts_match === 1 &&
      proof.unique_effects === 1
    );
  };
  const transact = (body: () => void) => {
    target.exec("BEGIN IMMEDIATE");
    try {
      body();
      target.exec("COMMIT");
    } catch (error) {
      target.exec("ROLLBACK");
      throw error;
    }
  };
  return {
    disconnectSource() {
      source?.close();
      source = null;
    },
    async apply(command: Command) {
      switch (command.kind) {
        case "abortRestore":
          transact(() => {
            const resume =
              hasVerifiedLocalImage() || fault === "abort-unverified";
            if (resume && fault === "abort-copies-staged-receipts") {
              const offered = target
                .prepare("SELECT receipts FROM ledger_offer")
                .get();
              assert.ok(offered);
              target.exec("DELETE FROM receipts");
              const insert = target.prepare("INSERT INTO receipts VALUES (?)");
              for (const frame of JSON.parse(
                String(offered.receipts),
              ) as string[])
                insert.run(frame);
            }
            target.exec("DELETE FROM fence_offer; DELETE FROM ledger_offer;");
            target.prepare("UPDATE control SET ready = ?").run(resume ? 1 : 0);
          });
          return;
        case "reopen":
          target.close();
          target = new DatabaseSync(targetPath);
          return;
        case "beginRestore":
          transact(() =>
            target.exec(
              "UPDATE control SET ready = 0; DELETE FROM fence_offer; DELETE FROM ledger_offer;",
            ),
          );
          return;
        case "receiveFence":
          transact(() => {
            const row = checkpoint(command.revision);
            target.exec("DELETE FROM fence_offer");
            target
              .prepare("INSERT INTO fence_offer VALUES (?, ?)")
              .run(command.revision, Number(row.closed));
          });
          return;
        case "receiveLedger":
          transact(() => {
            const row = checkpoint(command.revision);
            target.exec("DELETE FROM ledger_offer");
            target
              .prepare("INSERT INTO ledger_offer VALUES (?, ?, ?)")
              .run(command.revision, String(row.applied), String(row.receipts));
          });
          return;
        case "install":
          transact(() => {
            const current = control();
            const fence = target.prepare("SELECT * FROM fence_offer").get();
            const ledger = target.prepare("SELECT * FROM ledger_offer").get();
            if (
              !fence ||
              !ledger ||
              fence.revision !== current.required ||
              (fault !== "mixed-revisions" &&
                ledger.revision !== current.required)
            )
              return;
            const incoming = JSON.parse(String(ledger.applied)) as string[];
            if (
              (current.closed === 1 && fence.closed !== 1) ||
              (fault !== "ignore-local-effects" &&
                applied().some((frame) => !incoming.includes(frame)))
            )
              return;
            if (fault !== "ledger-only")
              target
                .prepare("UPDATE control SET closed = ?")
                .run(Number(fence.closed));
            if (fault !== "fence-only") {
              target.exec("DELETE FROM effects; DELETE FROM receipts;");
              const copyEffect = target.prepare(
                "INSERT INTO effects VALUES (?)",
              );
              for (const frame of incoming) copyEffect.run(frame);
              const copyReceipt = target.prepare(
                "INSERT INTO receipts VALUES (?)",
              );
              for (const frame of JSON.parse(
                String(ledger.receipts),
              ) as string[])
                copyReceipt.run(frame);
            }
            target.exec("DELETE FROM verified_effects");
            const remember = target.prepare(
              "INSERT INTO verified_effects VALUES (?)",
            );
            for (const frame of incoming) remember.run(frame);
            target
              .prepare("UPDATE control SET ready = 1, local_revision = ?")
              .run(String(ledger.revision));
          });
          return;
        case "submit":
          transact(() => {
            const { frame } = command;
            if (!control().ready) return;
            if (
              control().closed === 1 &&
              (frame === "OldOne" || frame === "OldTwo")
            )
              reply = "Expired";
            else if (
              target
                .prepare("SELECT 1 FROM receipts WHERE frame = ?")
                .get(frame)
            )
              reply = "Duplicate";
            else {
              target.prepare("INSERT INTO effects VALUES (?)").run(frame);
              target.prepare("INSERT INTO receipts VALUES (?)").run(frame);
              reply = "Applied";
            }
          });
          return;
      }
    },
    async observe() {
      const current = control();
      return {
        ready: current.ready === 1,
        closed: current.closed === 1,
        localRevision: current.local_revision,
        fenceOffer:
          target.prepare("SELECT revision FROM fence_offer").get()?.revision ??
          null,
        ledgerOffer:
          target.prepare("SELECT revision FROM ledger_offer").get()?.revision ??
          null,
        applied: applied(),
        receipts: target
          .prepare("SELECT frame FROM receipts ORDER BY frame")
          .all()
          .map((row) => row.frame),
        effects: target.prepare("SELECT COUNT(*) AS count FROM effects").get()
          ?.count,
        reply,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test("restore stages a matching checkpoint and preserves newer local effects", async (t) => {
  const initialAbort = await transitionScenario<Command>(
    checker,
    model,
    "abortRestore",
    "no before(localRevision) and some before(fenceOffer) and unchanged(ready) and no after(fenceOffer)",
  );
  assert.equal(initialAbort.commands.length, 2);
  await t.test(
    "aborting the first restore discards staging and keeps reception closed",
    async (t) => {
      const run = await recordReplay(
        checker,
        initialAbort,
        await openReplicas(t),
      );
      assert.equal(run.verdict, "matched");
      assert.deepEqual(run.recording.observations.at(-1), {
        ready: false,
        closed: false,
        localRevision: null,
        fenceOffer: null,
        ledgerOffer: null,
        applied: ["OldOne"],
        receipts: ["OldOne"],
        effects: 1,
        reply: null,
      });
    },
  );
  await t.test(
    "opening an unverified local image accepts previously applied work",
    async (t) => {
      const adapter = await openReplicas(t, "abort-unverified");
      const run = await recordReplay(checker, initialAbort, adapter);
      assert.equal(run.verdict, "mismatch");
      assert.deepEqual(run.difference.command, abortRestore);
      await adapter.apply(oldRetry);
      assert.equal((await adapter.observe()).reply, "Applied");
    },
  );
  const mixed = await transitionScenario<Command>(
    checker,
    model,
    "install",
    "before(fenceOffer) = AfterClose and before(ledgerOffer) = BeforeClose and unchanged(ready)",
  );
  assert.equal(mixed.commands.length, 3);
  const resumed = {
    ...mixed,
    commands: [
      ...mixed.commands,
      reopen,
      latestLedger,
      install,
      reopen,
      oldRetry,
      newRetry,
      fresh,
    ],
  };
  const laterRestore = {
    ...resumed,
    commands: [
      ...resumed.commands,
      { kind: "beginRestore" },
      latestFence,
      latestLedger,
      install,
    ] satisfies Command[],
  };
  const canceled = {
    ...laterRestore,
    commands: [
      ...laterRestore.commands,
      reopen,
      abortRestore,
      reopen,
      oldRetry,
      fresh,
    ],
  };
  await t.test(
    "a verified local image reopens with the source offline",
    async (t) => {
      const adapter = await openReplicas(t);
      const run = await recordReplay(checker, canceled, {
        ...adapter,
        async apply(command) {
          if (command.kind === "abortRestore") adapter.disconnectSource();
          await adapter.apply(command);
        },
      });
      assert.equal(run.verdict, "matched");
      const observed = run.recording.observations as { reply: string }[];
      assert.equal(observed.at(-2)?.reply, "Expired");
      assert.deepEqual(observed.at(-1), {
        ready: true,
        closed: true,
        localRevision: "AfterClose",
        fenceOffer: null,
        ledgerOffer: null,
        applied: ["NewOne", "NewTwo", "OldOne", "OldTwo"],
        receipts: ["NewOne", "NewTwo"],
        effects: 4,
        reply: "Duplicate",
      });
    },
  );
  await t.test(
    "abort must preserve the live receipts rather than copy staging",
    async (t) => {
      const adapter = await openReplicas(t, "abort-copies-staged-receipts");
      const run = await recordReplay(checker, canceled, adapter);
      assert.equal(run.verdict, "mismatch");
      assert.deepEqual(run.difference.command, abortRestore);
      await adapter.apply(reopen);
      await adapter.apply(fresh);
      assert.equal((await adapter.observe()).reply, "Applied");
      assert.equal((await adapter.observe()).effects, 5);
    },
  );
  await t.test(
    "mixed revisions wait and an older restore preserves the later write",
    async (t) => {
      const run = await recordReplay(
        checker,
        laterRestore,
        await openReplicas(t),
      );
      assert.equal(run.verdict, "matched");
      const observed = run.recording.observations as {
        ready: boolean;
        closed: boolean;
        effects: number;
        reply: string;
      }[];
      assert.equal(observed[mixed.commands.length]?.ready, false);
      assert.equal(observed[resumed.commands.length - 2]?.reply, "Expired");
      assert.equal(observed[resumed.commands.length - 1]?.reply, "Duplicate");
      assert.equal(observed[resumed.commands.length]?.reply, "Applied");
      assert.deepEqual(observed.at(-1), {
        ready: false,
        closed: true,
        localRevision: "AfterClose",
        fenceOffer: "AfterClose",
        ledgerOffer: "AfterClose",
        applied: ["NewOne", "NewTwo", "OldOne", "OldTwo"],
        receipts: ["NewOne", "NewTwo"],
        effects: 4,
        reply: "Applied",
      });
    },
  );
  await t.test(
    "mixing revisions publishes a fence without all its effects",
    async (t) => {
      const run = await recordReplay(
        checker,
        mixed,
        await openReplicas(t, "mixed-revisions"),
      );
      assert.equal(run.verdict, "mismatch");
      assert.equal(run.difference.step, mixed.commands.length);
      const observed = run.difference.observed as {
        ready: boolean;
        closed: boolean;
        applied: string[];
      };
      assert.equal(observed.ready, true);
      assert.equal(observed.closed, true);
      assert.deepEqual(observed.applied, ["OldOne"]);
    },
  );
  const published = await transitionScenario<Command>(
    checker,
    model,
    "install",
    "after(ready)",
  );
  assert.equal(published.commands.length, 3);
  for (const fault of ["fence-only", "ledger-only"] as const) {
    await t.test(fault, async (t) => {
      const adapter = await openReplicas(t, fault);
      const run = await recordReplay(checker, published, adapter);
      assert.equal(run.verdict, "mismatch");
      assert.equal(run.difference.step, 3);
      assert.deepEqual(run.difference.command, install);
      await adapter.apply(reopen);
      await adapter.apply(oldRetry);
      const observed = await adapter.observe();
      if (fault === "fence-only") {
        assert.equal(observed.reply, "Expired");
        assert.deepEqual(observed.applied, ["OldOne"]);
      } else {
        assert.equal(observed.reply, "Applied");
        assert.equal(observed.effects, 4);
      }
    });
  }
  await t.test(
    "matching source revisions cannot overwrite work applied after cutover",
    async (t) => {
      const run = await recordReplay(
        checker,
        laterRestore,
        await openReplicas(t, "ignore-local-effects"),
      );
      assert.equal(run.verdict, "mismatch");
      assert.equal(run.difference.step, laterRestore.commands.length);
      const observed = run.difference.observed as {
        ready: boolean;
        applied: string[];
      };
      assert.equal(observed.ready, true);
      assert.equal(observed.applied.includes("NewTwo"), false);
    },
  );
});
