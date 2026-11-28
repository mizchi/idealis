import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const positions = ["Zero", "One", "Two", "Three"] as const;
type Position = (typeof positions)[number];
type Command =
  | { kind: "read"; position: Position }
  | { kind: "append" | "capture" | "compact" | "install" | "advance" | "pull" };
type Fault = "live-head" | "drop-buffer" | null;

const checker = "crates/idealis-check/target/debug/idealis-check";
const model =
  "examples/model-patterns/subscriptions/replay-window-snapshot.ideal";
const install: Command = { kind: "install" };

function transaction(database: DatabaseSync, operation: () => void) {
  database.exec("BEGIN");
  try {
    operation();
    database.exec("COMMIT");
  } catch (error) {
    database.exec("ROLLBACK");
    throw error;
  }
}

async function openReplica(t: TestContext, fault: Fault) {
  const directory = await mkdtemp(join(tmpdir(), "idealis-replay-snapshot-"));
  const path = join(directory, "source.sqlite");
  const writer = new DatabaseSync(path);
  const snapshotReader = new DatabaseSync(path);
  const replica = new DatabaseSync(":memory:");
  t.after(async () => {
    snapshotReader.close();
    writer.close();
    replica.close();
    await rm(directory, { recursive: true, force: true });
  });

  writer.exec(`
    PRAGMA journal_mode = WAL;
    CREATE TABLE source_state (head INTEGER, floor INTEGER);
    INSERT INTO source_state VALUES (1, 1);
    CREATE TABLE image (seq INTEGER PRIMARY KEY);
    INSERT INTO image VALUES (1);
    CREATE TABLE feed (seq INTEGER PRIMARY KEY);
  `);
  replica.exec(`
    CREATE TABLE replica_state (cursor INTEGER, gap INTEGER);
    INSERT INTO replica_state VALUES (0, 1);
    CREATE TABLE buffered (seq INTEGER PRIMARY KEY);
    CREATE TABLE materialized (seq INTEGER PRIMARY KEY);
  `);
  const sourceState = () =>
    writer.prepare("SELECT head, floor FROM source_state").get() as {
      head: number;
      floor: number;
    };
  const replicaState = () =>
    replica.prepare("SELECT cursor, gap FROM replica_state").get() as {
      cursor: number;
      gap: number;
    };
  const enqueue = (seq: number) => {
    if (!writer.prepare("SELECT seq FROM feed WHERE seq = ?").get(seq))
      return false;
    replica.prepare("INSERT OR IGNORE INTO buffered VALUES (?)").run(seq);
    return true;
  };
  let captured: number | null = null;

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "append":
          transaction(writer, () => {
            writer.exec("UPDATE source_state SET head = head + 1");
            writer.exec("INSERT INTO image SELECT head FROM source_state");
            writer.exec("INSERT INTO feed SELECT head FROM source_state");
          });
          break;
        case "compact":
          transaction(writer, () => {
            writer.exec("UPDATE source_state SET floor = head");
            writer.exec("DELETE FROM feed");
          });
          break;
        case "capture": {
          assert.equal(captured, null);
          snapshotReader.exec("BEGIN");
          const row = snapshotReader
            .prepare("SELECT head FROM source_state")
            .get();
          assert.equal(typeof row?.head, "number");
          captured = Number(row?.head);
          break;
        }
        case "read":
          assert.ok(enqueue(positions.indexOf(command.position)));
          break;
        case "install": {
          assert.ok(captured !== null);
          const rows = snapshotReader
            .prepare("SELECT seq FROM image ORDER BY seq")
            .all();
          snapshotReader.exec("COMMIT");
          const cursor = fault === "live-head" ? sourceState().head : captured;
          transaction(replica, () => {
            replica.exec("DELETE FROM materialized");
            const insert = replica.prepare(
              "INSERT INTO materialized VALUES (?)",
            );
            for (const row of rows) insert.run(Number(row.seq));
            if (fault === "drop-buffer") replica.exec("DELETE FROM buffered");
            else
              replica
                .prepare("DELETE FROM buffered WHERE seq <= ?")
                .run(cursor);
            replica
              .prepare("UPDATE replica_state SET cursor = ?, gap = 0")
              .run(cursor);
          });
          captured = null;
          break;
        }
        case "advance": {
          const next = replicaState().cursor + 1;
          transaction(replica, () => {
            assert.equal(
              replica.prepare("DELETE FROM buffered WHERE seq = ?").run(next)
                .changes,
              1,
            );
            replica.prepare("INSERT INTO materialized VALUES (?)").run(next);
            replica.prepare("UPDATE replica_state SET cursor = ?").run(next);
          });
          break;
        }
        case "pull":
          replica
            .prepare("UPDATE replica_state SET gap = ?")
            .run(enqueue(replicaState().cursor + 1) ? 0 : 1);
          break;
      }
    },
    async observe() {
      const { head, floor } = sourceState();
      const { cursor, gap } = replicaState();
      const snapshotSet = (sql: string) =>
        replica
          .prepare(sql)
          .all()
          .map((row) => positions[Number(row.seq)])
          .sort();
      return {
        head: positions[head],
        floor: positions[floor],
        cursor: positions[cursor],
        gap: Boolean(gap),
        captured: captured === null ? null : positions[captured],
        buffered: snapshotSet("SELECT seq FROM buffered"),
        materialized: snapshotSet("SELECT seq FROM materialized"),
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "installing a pinned snapshot keeps already buffered future records",
  { timeout: 30_000 },
  async (t) => {
    const pending = await reachableScenario<Command>(
      checker,
      model,
      "SnapshotWithBufferedPrefixAndSuffix",
    );
    const prefix = [...pending.commands, { kind: "compact" } as const];

    await t.test(
      "the source can prune the future record while the pinned SQLite read stays open",
      async (t) => {
        const commands: readonly Command[] = [
          ...prefix,
          install,
          { kind: "advance" },
        ];
        const result = await recordReplay(
          checker,
          { ...pending, commands },
          await openReplica(t, null),
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-2), {
          head: "Three",
          floor: "Three",
          cursor: "Two",
          captured: null,
          buffered: ["Three"],
          materialized: ["One", "Two"],
          gap: false,
        });
        assert.deepEqual(result.recording.observations.at(-1), {
          head: "Three",
          floor: "Three",
          cursor: "Three",
          captured: null,
          buffered: [],
          materialized: ["One", "Three", "Two"],
          gap: false,
        });
      },
    );

    for (const fault of ["live-head", "drop-buffer"] as const) {
      await t.test(fault, async (t) => {
        const adapter = await openReplica(t, fault);
        const result = await recordReplay(
          checker,
          {
            ...pending,
            commands: [...prefix, install],
          },
          adapter,
        );
        assert.equal(result.verdict, "mismatch");
        assert.deepEqual(result.difference.command, install);
        assert.equal(result.difference.step, prefix.length + 1);
        const observed = await adapter.observe();
        assert.deepEqual(observed.materialized, ["One", "Two"]);
        if (fault === "live-head") {
          assert.equal(observed.cursor, "Three");
        } else {
          assert.deepEqual(observed.buffered, []);
          await adapter.apply({ kind: "pull" });
          assert.equal((await adapter.observe()).gap, true);
        }
      });
    }
  },
);
