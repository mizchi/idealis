import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  recordReplay,
  transitionScenario,
  type Scenario,
} from "../../examples/model-patterns/replay.js";

type Doctor = "Ada" | "Ben";
type Command = { kind: "inspect" | "submit"; doctor: Doctor };
type Outcome = "Committed" | "Conflict" | "Refused";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/sqlite-transactions/on-call.model";
const busySnapshot = 517;

const roster = (db: DatabaseSync): Doctor[] =>
  db
    .prepare("SELECT doctor FROM roster WHERE active = 1 ORDER BY doctor")
    .all()
    .map((row) => row.doctor as Doctor);

test(
  "WAL rejects a stale write across two duty rows and a fresh retry reevaluates the rule",
  { timeout: 30_000 },
  async (t) => {
    const directory = await mkdtemp(join(tmpdir(), "idealis-on-call-"));
    t.after(() => rm(directory, { recursive: true, force: true }));
    const faulty = join(directory, "unchecked.model");
    const source = await readFile(model, "utf8");
    const unchecked = source.replace(
      "if { doctor.basis = onDuty }",
      "if { true }",
    );
    assert.notEqual(unchecked, source);
    await writeFile(faulty, unchecked);

    const result = JSON.parse(
      execFileSync(
        checker,
        ["witness", faulty, "AtLeastOneDoctor", "--checker", "rprob"],
        { encoding: "utf8" },
      ),
    ) as {
      verdict: string;
      complete: boolean;
      trace: Omit<Scenario<Command>, "model">;
    };
    assert.equal(result.verdict, "violated");
    assert.equal(result.complete, true);
    const trace = result.trace;
    assert.deepEqual(trace.commands, [
      { kind: "inspect", doctor: "Ada" },
      { kind: "inspect", doctor: "Ben" },
      { kind: "submit", doctor: "Ada" },
      { kind: "submit", doctor: "Ben" },
    ]);
    const retry = await transitionScenario<Command>(
      checker,
      model,
      "submit",
      "doctor = Ben and before(doctor.outcome) = Conflict and after(doctor.outcome) = Refused",
    );
    assert.deepEqual(retry.commands.slice(0, 4), trace.commands);
    assert.equal(retry.commands.length, 6);
    const commands = retry.commands;

    for (const releaseBeforeWrite of [false, true]) {
      await t.test(
        releaseBeforeWrite
          ? "a separate UPDATE trusts the stale decision"
          : "the open read transaction rejects a stale UPDATE",
        async () => {
          const path = join(
            directory,
            releaseBeforeWrite ? "split.db" : "wal.db",
          );
          using observer = new DatabaseSync(path);
          assert.equal(
            observer.prepare("PRAGMA journal_mode = WAL").get()?.journal_mode,
            "wal",
          );
          observer.exec(`
        CREATE TABLE roster (doctor TEXT PRIMARY KEY, active INTEGER NOT NULL);
        INSERT INTO roster VALUES ('Ada', 1), ('Ben', 1)
      `);
          using ada = new DatabaseSync(path);
          using ben = new DatabaseSync(path);
          const connections = { Ada: ada, Ben: ben };
          const basis: Record<Doctor, Doctor[]> = { Ada: [], Ben: [] };
          const outcome: Record<Doctor, Outcome | null> = {
            Ada: null,
            Ben: null,
          };
          const codes: number[] = [];
          const run = await recordReplay(
            checker,
            {
              model,
              config: trace.config,
              initial: trace.initial,
              commands,
            },
            {
              async apply({ kind, doctor }) {
                const db = connections[doctor];
                if (kind === "inspect") {
                  db.exec("BEGIN");
                  basis[doctor] = roster(db);
                  if (releaseBeforeWrite) db.exec("COMMIT");
                  return;
                }
                try {
                  if (basis[doctor].length < 2) {
                    if (!releaseBeforeWrite) db.exec("ROLLBACK");
                    outcome[doctor] = "Refused";
                    return;
                  }
                  db.prepare(
                    "UPDATE roster SET active = 0 WHERE doctor = ?",
                  ).run(doctor);
                  if (!releaseBeforeWrite) db.exec("COMMIT");
                  outcome[doctor] = "Committed";
                } catch (error) {
                  if (
                    releaseBeforeWrite ||
                    !(error instanceof Error) ||
                    !("errcode" in error) ||
                    error.errcode !== busySnapshot
                  )
                    throw error;
                  codes.push(error.errcode);
                  db.exec("ROLLBACK");
                  outcome[doctor] = "Conflict";
                } finally {
                  basis[doctor] = [];
                }
              },
              async observe() {
                return {
                  onDuty: roster(observer),
                  basis: { ...basis },
                  outcome: { ...outcome },
                };
              },
              project: (state) => state,
            },
          );
          const expected = {
            onDuty: ["Ben"],
            basis: { Ada: [], Ben: [] },
            outcome: { Ada: "Committed", Ben: "Conflict" },
          };
          if (releaseBeforeWrite) {
            assert.equal(run.verdict, "mismatch");
            assert.deepEqual(run.difference, {
              step: 4,
              command: commands[3],
              expected,
              observed: {
                ...expected,
                onDuty: [],
                outcome: { Ada: "Committed", Ben: "Committed" },
              },
            });
            assert.deepEqual(codes, []);
          } else {
            assert.equal(run.verdict, "matched");
            assert.deepEqual(codes, [busySnapshot]);
            assert.deepEqual(run.recording.observations[4], expected);
            assert.deepEqual(run.recording.observations[5], {
              ...expected,
              basis: { Ada: [], Ben: ["Ben"] },
            });
            assert.deepEqual(run.recording.observations.at(-1), {
              ...expected,
              outcome: { Ada: "Committed", Ben: "Refused" },
            });
          }
        },
      );
    }
  },
);

type SnapshotCommand = { kind: "inspect" | "pause" | "resume" | "submit" };
const abaModel =
  "examples/model-patterns/sqlite-transactions/snapshot-aba.model";
const epochs = ["V0", "V1", "V2", "V3"];

test(
  "identical duty rows can belong to a different database revision",
  { timeout: 30_000 },
  async (t) => {
    const directory = await mkdtemp(join(tmpdir(), "idealis-roster-aba-"));
    t.after(() => rm(directory, { recursive: true, force: true }));
    const faulty = join(directory, "values-only.model");
    const source = await readFile(abaModel, "utf8");
    const unchecked = source.replace(
      "if { captured = epoch }",
      "if { basis = onDuty }",
    );
    assert.notEqual(unchecked, source);
    await writeFile(faulty, unchecked);
    const result = JSON.parse(
      execFileSync(
        checker,
        [
          "witness",
          faulty,
          "RestoringRowsDoesNotRestoreTheSnapshot",
          "--checker",
          "rprob",
        ],
        { encoding: "utf8" },
      ),
    ) as {
      verdict: string;
      complete: boolean;
      trace: Omit<Scenario<SnapshotCommand>, "model">;
    };
    assert.equal(result.verdict, "violated");
    assert.equal(result.complete, true);
    const trace = result.trace;
    assert.deepEqual(
      trace.commands.map(({ kind }) => kind),
      ["inspect", "pause", "resume", "submit"],
    );

    for (const [mode, stale] of [
      ["wal", true],
      ["version", true],
      ["values-only", true],
      ["version", false],
    ] as const) {
      const name = stale ? mode : "version with a current snapshot";
      await t.test(name, async () => {
        const path = join(directory, name + ".db");
        using admin = new DatabaseSync(path);
        assert.equal(
          admin.prepare("PRAGMA journal_mode = WAL").get()?.journal_mode,
          "wal",
        );
        admin.exec(`
        CREATE TABLE roster (doctor TEXT PRIMARY KEY, active INTEGER NOT NULL);
        INSERT INTO roster VALUES ('Ada', 1), ('Ben', 1);
        CREATE TABLE epoch (version INTEGER NOT NULL);
        INSERT INTO epoch VALUES (0);
        CREATE TRIGGER bump_epoch AFTER UPDATE ON roster BEGIN
          UPDATE epoch SET version = version + 1;
        END;
      `);
        using doctor = new DatabaseSync(path);
        const version = (db: DatabaseSync) => {
          const value = db.prepare("SELECT version FROM epoch").get()?.version;
          assert.ok(typeof value === "number");
          return value;
        };
        let basis: Doctor[] = [];
        let captured: number | null = null;
        let outcome: Outcome | null = null;
        const codes: number[] = [];
        const commands: readonly SnapshotCommand[] = stale
          ? trace.commands
          : [{ kind: "inspect" }, { kind: "submit" }];
        const run = await recordReplay(
          checker,
          { ...trace, model: abaModel, commands },
          {
            async apply({ kind }) {
              switch (kind) {
                case "inspect":
                  doctor.exec("BEGIN");
                  basis = roster(doctor);
                  captured = version(doctor);
                  if (mode !== "wal") doctor.exec("COMMIT");
                  return;
                case "pause":
                  admin.exec(
                    "UPDATE roster SET active = 0 WHERE doctor = 'Ben'",
                  );
                  return;
                case "resume":
                  admin.exec(
                    "UPDATE roster SET active = 1 WHERE doctor = 'Ben'",
                  );
                  return;
                case "submit":
                  if (mode !== "wal") {
                    doctor.exec("BEGIN IMMEDIATE");
                    const fresh =
                      mode === "version"
                        ? captured === version(doctor)
                        : JSON.stringify(basis) ===
                          JSON.stringify(roster(doctor));
                    if (!fresh) {
                      doctor.exec("ROLLBACK");
                      outcome = "Conflict";
                      break;
                    }
                  }
                  try {
                    doctor.exec(
                      "UPDATE roster SET active = 0 WHERE doctor = 'Ada'; COMMIT",
                    );
                    outcome = "Committed";
                  } catch (error) {
                    if (
                      mode !== "wal" ||
                      !(error instanceof Error) ||
                      !("errcode" in error) ||
                      error.errcode !== busySnapshot
                    )
                      throw error;
                    codes.push(error.errcode);
                    doctor.exec("ROLLBACK");
                    outcome = "Conflict";
                  }
                  break;
              }
              captured = null;
              basis = [];
            },
            async observe() {
              return {
                onDuty: roster(admin),
                epoch: epochs[version(admin)],
                captured: captured === null ? null : epochs[captured],
                basis: [...basis],
                outcome,
              };
            },
            project: (state) => state,
          },
        );
        if (!stale) {
          assert.equal(run.verdict, "matched");
          assert.deepEqual(codes, []);
          assert.deepEqual(run.recording.observations.at(-1), {
            onDuty: ["Ben"],
            epoch: "V1",
            captured: null,
            basis: [],
            outcome: "Committed",
          });
          return;
        }
        assert.deepEqual(run.recording.observations[3], {
          onDuty: ["Ada", "Ben"],
          epoch: "V2",
          captured: "V0",
          basis: ["Ada", "Ben"],
          outcome: null,
        });
        const expected = {
          onDuty: ["Ada", "Ben"],
          epoch: "V2",
          captured: null,
          basis: [],
          outcome: "Conflict",
        };
        assert.deepEqual(codes, mode === "wal" ? [busySnapshot] : []);
        if (mode === "values-only") {
          assert.equal(run.verdict, "mismatch");
          assert.deepEqual(run.difference, {
            step: 4,
            command: { kind: "submit" },
            expected,
            observed: {
              ...expected,
              onDuty: ["Ben"],
              epoch: "V3",
              outcome: "Committed",
            },
          });
        } else {
          assert.equal(run.verdict, "matched");
          assert.deepEqual(run.recording.observations.at(-1), expected);
        }
      });
    }
  },
);
