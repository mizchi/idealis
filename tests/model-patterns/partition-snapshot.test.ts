import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import { promisify } from "node:util";
import {
  reachableScenario,
  recordReplay,
  type Scenario,
} from "../../examples/model-patterns/replay.js";

const partitions = ["Orders", "Payments", "Audit"] as const;
const positions = ["Zero", "One", "Two", "Three"] as const;
type Partition = (typeof partitions)[number];
type Position = (typeof positions)[number];
type EntryPosition = Exclude<Position, "Zero">;
type Command =
  | { kind: "append" | "capture" | "advance"; partition: Partition }
  | { kind: "read"; partition: Partition; position: EntryPosition }
  | { kind: "publish" };
type Fault = "ignore-dependency" | "shared-cursor" | "two-edges" | null;
type Entry = { part: Partition; seq: number; requires_order: number | null };

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/partition-snapshot.model";
const publish: Command = { kind: "publish" };

function openReplica(t: TestContext, fault: Fault) {
  const source = new DatabaseSync(":memory:");
  const replica = new DatabaseSync(":memory:");
  t.after(() => {
    source.close();
    replica.close();
  });
  source.exec(`
    CREATE TABLE events (part TEXT, seq INTEGER, requires_order INTEGER, PRIMARY KEY (part, seq));
    INSERT INTO events VALUES ('Orders', 1, NULL);
  `);
  replica.exec(`
    CREATE TABLE cursors (part TEXT PRIMARY KEY, seq INTEGER);
    INSERT INTO cursors VALUES ('Orders', 0), ('Payments', 0), ('Audit', 0);
    CREATE TABLE buffered (part TEXT, seq INTEGER, requires_order INTEGER, PRIMARY KEY (part, seq));
    CREATE TABLE applied (part TEXT, seq INTEGER, PRIMARY KEY (part, seq));
  `);
  const frozen = new Map<Partition, { position: number; entries: Entry[] }>();
  let decision: "Waiting" | "Accepted" | "Rejected" = "Waiting";
  const head = (partition: Partition) =>
    Number(
      source
        .prepare(
          "SELECT COALESCE(MAX(seq), 0) AS seq FROM events WHERE part = ?",
        )
        .get(partition)?.seq,
    );
  const cursor = (partition: Partition) =>
    Number(
      replica.prepare("SELECT seq FROM cursors WHERE part = ?").get(partition)
        ?.seq,
    );
  function commit(operation: () => void) {
    replica.exec("BEGIN");
    try {
      operation();
      replica.exec("COMMIT");
    } catch (error) {
      replica.exec("ROLLBACK");
      throw error;
    }
  }
  const byPartition = <V>(get: (part: Partition) => V) => ({
    Orders: get("Orders"),
    Payments: get("Payments"),
    Audit: get("Audit"),
  });

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "append": {
          const { partition } = command;
          source
            .prepare("INSERT INTO events VALUES (?, ?, ?)")
            .run(
              partition,
              head(partition) + 1,
              partition === "Payments" ? head("Orders") : null,
            );
          break;
        }
        case "capture": {
          const { partition } = command;
          const entries = source
            .prepare("SELECT * FROM events WHERE part = ? ORDER BY seq")
            .all(partition) as Entry[];
          frozen.set(partition, {
            position: entries.at(-1)?.seq ?? 0,
            entries,
          });
          break;
        }
        case "read": {
          const entry = source
            .prepare("SELECT * FROM events WHERE part = ? AND seq = ?")
            .get(command.partition, positions.indexOf(command.position)) as
            | Entry
            | undefined;
          assert.ok(entry);
          replica
            .prepare("INSERT OR IGNORE INTO buffered VALUES (?, ?, ?)")
            .run(entry.part, entry.seq, entry.requires_order);
          break;
        }
        case "publish": {
          const entries = [...frozen.values()].flatMap(
            (snapshot) => snapshot.entries,
          );
          const orders = new Set(
            entries
              .filter((row) => row.part === "Orders")
              .map((row) => row.seq),
          );
          const containsDependencies = entries.every(
            (row) =>
              row.requires_order === null || orders.has(row.requires_order),
          );
          if (!containsDependencies && fault !== "ignore-dependency") {
            decision = "Rejected";
            frozen.clear();
            break;
          }
          commit(() => {
            replica.exec("DELETE FROM applied");
            const insert = replica.prepare("INSERT INTO applied VALUES (?, ?)");
            for (const row of entries) insert.run(row.part, row.seq);
            const writeCursor = replica.prepare(
              "UPDATE cursors SET seq = ? WHERE part = ?",
            );
            const maximum = Math.max(
              ...[...frozen.values()].map((snapshot) => snapshot.position),
            );
            for (const [part, snapshot] of frozen) {
              writeCursor.run(
                fault === "shared-cursor" ? maximum : snapshot.position,
                part,
              );
            }
            replica.exec(
              "DELETE FROM buffered WHERE seq <= (SELECT seq FROM cursors WHERE part = buffered.part)",
            );
            if (fault === "two-edges") {
              replica.exec(
                "DELETE FROM buffered WHERE seq > (SELECT seq + 2 FROM cursors WHERE part = buffered.part)",
              );
            }
          });
          decision = "Accepted";
          frozen.clear();
          break;
        }
        case "advance": {
          const { partition } = command;
          const next = cursor(partition) + 1;
          commit(() => {
            const entry = replica
              .prepare("SELECT * FROM buffered WHERE part = ? AND seq = ?")
              .get(partition, next) as Entry | undefined;
            assert.ok(entry);
            if (entry.requires_order !== null) {
              assert.ok(
                replica
                  .prepare(
                    "SELECT seq FROM applied WHERE part = 'Orders' AND seq = ?",
                  )
                  .get(entry.requires_order),
              );
            }
            replica
              .prepare("INSERT INTO applied VALUES (?, ?)")
              .run(partition, next);
            replica
              .prepare("DELETE FROM buffered WHERE part = ? AND seq = ?")
              .run(partition, next);
            replica
              .prepare("UPDATE cursors SET seq = ? WHERE part = ?")
              .run(next, partition);
          });
          break;
        }
      }
    },
    async observe() {
      const queue = replica.prepare(
        "SELECT seq FROM buffered WHERE part = ? ORDER BY seq",
      );
      const applied = replica.prepare(
        "SELECT seq FROM applied WHERE part = ? ORDER BY seq",
      );
      return {
        head: byPartition((part) => positions[head(part)]),
        cursor: byPartition((part) => positions[cursor(part)]),
        captured: byPartition((part) =>
          frozen.has(part) ? positions[frozen.get(part)!.position] : null,
        ),
        buffered: byPartition((part) =>
          queue
            .all(part)
            .map((row) => positions[Number(row.seq)])
            .sort(),
        ),
        materialized: byPartition((part) =>
          applied.all(part).map((row) => positions[Number(row.seq)]),
        ),
        decision,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => {
      const cursor = state.cursor as Record<Partition, Position>;
      return {
        ...state,
        materialized: byPartition((part) =>
          positions.slice(1, positions.indexOf(cursor[part]) + 1),
        ),
      };
    },
  };
}

test("both source branches of snapshot publication replay against SQLite", async (t) => {
  const { stdout } = await promisify(execFile)(checker, [
    "branches",
    model,
    "publish",
    "--max-states",
    "20000",
  ]);
  type Arm = {
    verdict: string;
    trace: Omit<Scenario<Command>, "model"> | null;
  };
  const report = JSON.parse(stdout) as {
    complete: boolean;
    branches: readonly { then: Arm; else: Arm }[];
  };
  assert.equal(report.complete, true);
  assert.equal(report.branches.length, 1);
  const group = report.branches[0]!;
  for (const [arm, decision] of [
    [group.then, "Accepted"],
    [group.else, "Rejected"],
  ] as const) {
    assert.equal(arm.verdict, "reachable");
    assert.ok(arm.trace);
    const scenario = { ...arm.trace, model };
    assert.deepEqual(scenario.commands.at(-1), publish);
    const result = await recordReplay(checker, scenario, openReplica(t, null));
    assert.equal(result.verdict, "matched");
    assert.equal(
      (result.recording.observations.at(-1) as { decision: string }).decision,
      decision,
    );
    if (decision === "Rejected") {
      const faulty = await recordReplay(
        checker,
        scenario,
        openReplica(t, "ignore-dependency"),
      );
      assert.equal(faulty.verdict, "mismatch");
      assert.equal(faulty.difference.step, scenario.commands.length);
      assert.equal(
        (faulty.difference.expected as { decision: string }).decision,
        "Rejected",
      );
      assert.equal(
        (faulty.difference.observed as { decision: string }).decision,
        "Accepted",
      );
    }
  }
});

test(
  "a partition snapshot contains the causal predecessors of all its rows",
  { timeout: 30_000 },
  async (t) => {
    const crossed = await reachableScenario<Command>(
      checker,
      model,
      "InconsistentCut",
    );
    const skewed = await reachableScenario<Command>(
      checker,
      model,
      "DifferentOffsetsWithBufferedSuffix",
    );
    const repair: readonly Command[] = [
      publish,
      { kind: "capture", partition: "Orders" },
      { kind: "capture", partition: "Payments" },
      { kind: "capture", partition: "Audit" },
      publish,
    ];
    await t.test(
      "rejects a crossed cut and recaptures the missing predecessor",
      async (t) => {
        const result = await recordReplay(
          checker,
          { ...crossed, commands: [...crossed.commands, ...repair] },
          openReplica(t, null),
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(
          result.recording.observations[crossed.commands.length + 1],
          {
            head: { Orders: "Two", Payments: "One", Audit: "Zero" },
            cursor: { Orders: "Zero", Payments: "Zero", Audit: "Zero" },
            buffered: { Orders: [], Payments: [], Audit: [] },
            captured: { Orders: null, Payments: null, Audit: null },
            materialized: { Orders: [], Payments: [], Audit: [] },
            decision: "Rejected",
          },
        );
        assert.deepEqual(
          (result.recording.observations.at(-1) as { materialized: object })
            .materialized,
          {
            Orders: ["One", "Two"],
            Payments: ["One"],
            Audit: [],
          },
        );
      },
    );
    await t.test(
      "equal numeric buffer offsets can be on different sides of the cut",
      async (t) => {
        const result = await recordReplay(
          checker,
          {
            ...skewed,
            commands: [
              ...skewed.commands,
              publish,
              { kind: "advance", partition: "Payments" },
              { kind: "read", partition: "Audit", position: "One" },
              { kind: "advance", partition: "Audit" },
              { kind: "read", partition: "Audit", position: "Two" },
              { kind: "advance", partition: "Audit" },
              { kind: "advance", partition: "Audit" },
            ],
          },
          openReplica(t, null),
        );
        assert.equal(result.verdict, "matched");
        const installed = result.recording.observations[
          skewed.commands.length + 1
        ] as {
          buffered: object;
          cursor: object;
        };
        assert.deepEqual(installed.buffered, {
          Orders: [],
          Payments: ["One"],
          Audit: ["Three"],
        });
        assert.deepEqual(installed.cursor, {
          Orders: "Two",
          Payments: "Zero",
          Audit: "Zero",
        });
        assert.deepEqual(
          (result.recording.observations.at(-1) as { cursor: object }).cursor,
          { Orders: "Two", Payments: "One", Audit: "Three" },
        );
      },
    );
    for (const [fault, scenario] of [
      ["ignore-dependency", crossed],
      ["shared-cursor", skewed],
      ["two-edges", skewed],
    ] as const) {
      await t.test(fault, async (t) => {
        const result = await recordReplay(
          checker,
          {
            ...scenario,
            commands: [...scenario.commands, publish],
          },
          openReplica(t, fault),
        );
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, scenario.commands.length + 1);
        assert.deepEqual(result.difference.command, publish);
        const actual = result.difference.observed as {
          cursor: object;
          materialized: object;
          buffered: object;
        };
        if (fault === "ignore-dependency")
          assert.deepEqual(actual.materialized, {
            Orders: ["One"],
            Payments: ["One"],
            Audit: [],
          });
        else if (fault === "shared-cursor")
          assert.deepEqual(actual.cursor, {
            Orders: "Two",
            Payments: "Two",
            Audit: "Two",
          });
        else
          assert.deepEqual(actual.buffered, {
            Orders: [],
            Payments: ["One"],
            Audit: [],
          });
      });
    }
  },
);
