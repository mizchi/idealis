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
type Envelope = { registration: Registration; sequence: Sequence };
type Command =
  | { kind: "rotate" | "retire" | "crash" | "restart" }
  | ({ kind: "receive" | "ack" } & Envelope);
type Fault = "sequence-only" | "use-active-registration";
const registrations = ["New", "Old"] as const;
const sequences = ["First", "Second"] as const;
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/crash-recovery/decoder-identity.ideal";

async function openReceiver(t: TestContext, fault?: Fault) {
  const dir = await mkdtemp(join(tmpdir(), "idealis-decoder-identity-"));
  const path = join(dir, "receiver.sqlite");
  const observer = new DatabaseSync(path);
  let receiver: DatabaseSync | undefined = new DatabaseSync(path);
  t.after(async () => {
    receiver?.close();
    observer.close();
    await rm(dir, { recursive: true, force: true });
  });
  observer.exec(`
    CREATE TABLE progress (active TEXT NOT NULL, retired INTEGER NOT NULL);
    INSERT INTO progress VALUES ('Old', 0);
    CREATE TABLE receipts (registration TEXT, sequence TEXT, PRIMARY KEY (registration, sequence));
    CREATE TABLE effects (registration TEXT, sequence TEXT, decoded INTEGER NOT NULL);
    CREATE TABLE dropped (registration TEXT, sequence TEXT, PRIMARY KEY (registration, sequence));
  `);

  let life: "Online" | "Offline" | "Restarted" = "Online";
  const pending = new Map<Registration, Set<Sequence>>();
  const confirmed = new Map<Registration, Set<Sequence>>();
  for (const registration of registrations) {
    pending.set(registration, new Set());
    confirmed.set(registration, new Set());
  }
  // Transport metadata remains available to the receiver independently of payload decoding.
  const frames = new Map(
    registrations.map((r) => [
      r,
      new Map(sequences.map((s) => [s, [r === "Old" ? 1 : 2, 1] as const])),
    ]),
  );
  function rows(table: "receipts" | "dropped", registration: Registration) {
    return observer
      .prepare(
        `SELECT sequence FROM ${table} WHERE registration = ? ORDER BY sequence`,
      )
      .all(registration)
      .map((row) => row.sequence);
  }
  return {
    async apply(command: Command) {
      if (command.kind === "restart") {
        assert.equal(receiver, undefined);
        receiver = new DatabaseSync(path);
        life = "Restarted";
        return;
      }
      assert.ok(receiver);
      switch (command.kind) {
        case "crash":
          receiver.close();
          receiver = undefined;
          for (const set of pending.values()) set.clear();
          life = "Offline";
          break;
        case "rotate":
          receiver.exec("UPDATE progress SET active = 'New'");
          break;
        case "retire":
          receiver.exec(`UPDATE progress SET retired = 1 WHERE active = 'New'
            AND 2 = (SELECT COUNT(*) FROM receipts WHERE registration = 'Old')`);
          break;
        case "receive": {
          const { registration, sequence } = command;
          const bytes = frames.get(registration)?.get(sequence);
          assert.ok(bytes);
          const policy = receiver
            .prepare("SELECT active, retired FROM progress")
            .get();
          assert.ok(policy);
          const duplicate =
            fault === "sequence-only"
              ? receiver
                  .prepare("SELECT 1 FROM receipts WHERE sequence = ?")
                  .get(sequence)
              : receiver
                  .prepare(
                    "SELECT 1 FROM receipts WHERE registration = ? AND sequence = ?",
                  )
                  .get(
                    fault === "use-active-registration"
                      ? policy.active!
                      : registration,
                    sequence,
                  );
          if (!duplicate) {
            const [version, value] = bytes;
            if (policy.retired === 1 && version === 1) {
              receiver
                .prepare("INSERT OR IGNORE INTO dropped VALUES (?, ?)")
                .run(registration, sequence);
              break;
            }
            receiver.exec("BEGIN");
            try {
              receiver
                .prepare("INSERT INTO receipts VALUES (?, ?)")
                .run(registration, sequence);
              receiver
                .prepare("INSERT INTO effects VALUES (?, ?, ?)")
                .run(registration, sequence, value);
              receiver.exec("COMMIT");
            } catch (error) {
              receiver.exec("ROLLBACK");
              throw error;
            }
          }
          pending.get(registration)?.add(sequence);
          break;
        }
        case "ack":
          assert.ok(
            pending.get(command.registration)?.delete(command.sequence),
          );
          assert.ok(frames.get(command.registration)?.delete(command.sequence));
          confirmed.get(command.registration)?.add(command.sequence);
          break;
      }
    },
    async observe() {
      const policy = observer
        .prepare("SELECT active, retired FROM progress")
        .get();
      assert.ok(policy);
      return {
        life,
        active: policy.active,
        decoder: policy.retired ? "CurrentOnly" : "Both",
        effects: observer.prepare("SELECT COUNT(*) AS count FROM effects").get()
          ?.count,
        seen: Object.fromEntries(
          registrations.map((r) => [r, rows("receipts", r)]),
        ),
        dropped: Object.fromEntries(
          registrations.map((r) => [r, rows("dropped", r)]),
        ),
        pendingAck: Object.fromEntries(
          registrations.map((r) => [r, [...pending.get(r)!].sort()]),
        ),
        acked: Object.fromEntries(
          registrations.map((r) => [r, [...confirmed.get(r)!].sort()]),
        ),
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "a reused sequence is deduplicated within its registration",
  { timeout: 30_000 },
  async (t) => {
    await t.test(
      "the new registration's first frame must take effect",
      async (t) => {
        const recovered = await transitionScenario<Command>(
          checker,
          model,
          "restart",
          "after(active) = New and before(Old.seen) = First and no before(New.seen) and no before(acked)",
        );
        const scenario = {
          ...recovered,
          commands: [
            ...recovered.commands,
            {
              kind: "receive",
              registration: "New",
              sequence: "First",
            } as const,
          ],
        };
        const run = await recordReplay(
          checker,
          scenario,
          await openReceiver(t),
        );
        assert.equal(run.verdict, "matched");
        assert.deepEqual(run.recording.observations.at(-1), {
          life: "Restarted",
          active: "New",
          decoder: "Both",
          effects: 2,
          seen: { Old: ["First"], New: ["First"] },
          dropped: { Old: [], New: [] },
          pendingAck: { Old: [], New: ["First"] },
          acked: { Old: [], New: [] },
        });
        const faulty = await recordReplay(
          checker,
          scenario,
          await openReceiver(t, "sequence-only"),
        );
        assert.equal(faulty.verdict, "mismatch");
        assert.equal(faulty.difference.step, scenario.commands.length);
        assert.equal(
          (faulty.difference.observed as { effects: number }).effects,
          1,
        );
      },
    );

    await t.test(
      "an old retry finds its own receipt after the decoder is retired",
      async (t) => {
        const scenario = await transitionScenario<Command>(
          checker,
          model,
          "restart",
          "after(active) = New and after(decoder) = CurrentOnly and no before(acked) and no before(New.seen)",
        );
        const withRetry = {
          ...scenario,
          commands: [
            ...scenario.commands,
            {
              kind: "receive",
              registration: "Old",
              sequence: "First",
            } as const,
          ],
        };
        const run = await recordReplay(
          checker,
          {
            ...withRetry,
            commands: [
              ...withRetry.commands,
              { kind: "receive", registration: "New", sequence: "First" },
              { kind: "ack", registration: "New", sequence: "First" },
              { kind: "ack", registration: "Old", sequence: "First" },
            ],
          },
          await openReceiver(t),
        );
        assert.equal(run.verdict, "matched");
        assert.deepEqual(run.recording.observations.at(-1), {
          life: "Restarted",
          active: "New",
          decoder: "CurrentOnly",
          effects: 3,
          seen: { Old: ["First", "Second"], New: ["First"] },
          dropped: { Old: [], New: [] },
          pendingAck: { Old: [], New: [] },
          acked: { Old: ["First"], New: ["First"] },
        });
        const faulty = await recordReplay(
          checker,
          withRetry,
          await openReceiver(t, "use-active-registration"),
        );
        assert.equal(faulty.verdict, "mismatch");
        assert.equal(faulty.difference.step, withRetry.commands.length);
        assert.deepEqual(
          (faulty.difference.observed as { dropped: object }).dropped,
          { Old: ["First"], New: [] },
        );
      },
    );
  },
);
