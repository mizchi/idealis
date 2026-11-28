import assert from "node:assert/strict";
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
  | {
      kind: "advance" | "acknowledge" | "trim" | "expire" | "pull" | "install";
    };

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/subscriptions/replay-window.ideal";

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

function openWindow(t: TestContext, fault: "maximum-as-prefix" | null) {
  const source = new DatabaseSync(":memory:");
  const replica = new DatabaseSync(":memory:");
  t.after(() => {
    source.close();
    replica.close();
  });
  source.exec(`
    CREATE TABLE feed (seq INTEGER PRIMARY KEY);
    INSERT INTO feed VALUES (1), (2), (3);
    CREATE TABLE snapshot (seq INTEGER PRIMARY KEY);
    INSERT INTO snapshot SELECT seq FROM feed;
    CREATE TABLE state (head INTEGER, floor INTEGER, ack INTEGER);
    INSERT INTO state VALUES (3, 0, 0);
  `);
  replica.exec(`
    CREATE TABLE state (cursor INTEGER, gap INTEGER);
    INSERT INTO state VALUES (0, 0);
    CREATE TABLE buffer (seq INTEGER PRIMARY KEY);
  `);
  const observed = new Set<number>();
  const local = () =>
    replica.prepare("SELECT cursor, gap FROM state").get() as {
      cursor: number;
      gap: number;
    };
  const remote = () =>
    source.prepare("SELECT head, floor, ack FROM state").get() as {
      head: number;
      floor: number;
      ack: number;
    };
  const receive = (seq: number) => {
    const row = source.prepare("SELECT seq FROM feed WHERE seq = ?").get(seq);
    if (row === undefined) return false;
    replica
      .prepare(
        "INSERT OR IGNORE INTO buffer SELECT ? FROM state WHERE ? > cursor",
      )
      .run(seq, seq);
    observed.add(seq);
    return true;
  };
  const prune = (through: number) =>
    transaction(source, () => {
      source.prepare("DELETE FROM feed WHERE seq <= ?").run(through);
      source.prepare("UPDATE state SET floor = ?").run(through);
    });

  return {
    async apply(command: Command) {
      const { cursor } = local();
      switch (command.kind) {
        case "read":
          assert.ok(receive(positions.indexOf(command.position)));
          break;
        case "advance":
          transaction(replica, () => {
            assert.equal(
              replica
                .prepare("DELETE FROM buffer WHERE seq = ?")
                .run(cursor + 1).changes,
              1,
            );
            replica.prepare("UPDATE state SET cursor = ?").run(cursor + 1);
          });
          break;
        case "acknowledge": {
          let ack = cursor;
          if (fault === "maximum-as-prefix") {
            const row = replica
              .prepare("SELECT MAX(seq) AS seq FROM buffer")
              .get();
            ack = Math.max(cursor, Number(row?.seq));
          }
          source.prepare("UPDATE state SET ack = ?").run(ack);
          break;
        }
        case "trim":
          prune(remote().ack);
          break;
        case "expire":
          prune(remote().floor + 1);
          break;
        case "pull":
          replica
            .prepare("UPDATE state SET gap = ?")
            .run(receive(cursor + 1) ? 0 : 1);
          break;
        case "install": {
          const { head } = remote();
          const snapshot = source
            .prepare("SELECT seq FROM snapshot ORDER BY seq")
            .all();
          assert.equal(snapshot.length, head);
          assert.ok(
            snapshot.every((record, index) => record.seq === index + 1),
          );
          transaction(replica, () => {
            replica.prepare("UPDATE state SET cursor = ?, gap = 0").run(head);
            replica.exec("DELETE FROM buffer");
          });
          for (const record of snapshot) observed.add(Number(record.seq));
          break;
        }
      }
    },
    async observe() {
      const { cursor, gap } = local();
      const { floor, ack } = remote();
      const sequence = (values: Iterable<number>) =>
        Array.from(values, (seq) => positions[seq]).sort();
      return {
        cursor: positions[cursor],
        floor: positions[floor],
        ack: positions[ack],
        buffered: sequence(
          replica
            .prepare("SELECT seq FROM buffer")
            .all()
            .map((row) => Number(row.seq)),
        ),
        retained: sequence(
          source
            .prepare("SELECT seq FROM feed")
            .all()
            .map((row) => Number(row.seq)),
        ),
        received: sequence(observed),
        gap: Boolean(gap),
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "replay retention follows the contiguous prefix even when later records arrived",
  { timeout: 30_000 },
  async (t) => {
    const hole = await reachableScenario<Command>(
      checker,
      model,
      "LaterRecordWithGap",
    );
    const expired = await reachableScenario<Command>(
      checker,
      model,
      "MissingPositionExpired",
    );
    const ack: Command = { kind: "acknowledge" };
    const trim: Command = { kind: "trim" };
    const pull: Command = { kind: "pull" };
    const advance: Command = { kind: "advance" };
    const install: Command = { kind: "install" };
    await t.test(
      "trim leaves the gap available and a duplicate remains idempotent",
      async (t) => {
        const commands: readonly Command[] = [
          ...hole.commands,
          ack,
          trim,
          { kind: "read", position: "Three" },
          pull,
          advance,
          advance,
        ];
        const result = await recordReplay(
          checker,
          { ...hole, commands },
          openWindow(t, null),
        );
        assert.equal(result.verdict, "matched", JSON.stringify(result));
        const afterTrim =
          result.recording.observations[hole.commands.length + 2];
        assert.deepEqual(afterTrim, {
          cursor: "One",
          floor: "One",
          ack: "One",
          buffered: ["Three"],
          retained: ["Three", "Two"],
          received: ["One", "Three"],
          gap: false,
        });
        assert.deepEqual(result.recording.observations.at(-1), {
          cursor: "Three",
          floor: "One",
          ack: "One",
          buffered: [],
          retained: ["Three", "Two"],
          received: ["One", "Three", "Two"],
          gap: false,
        });
      },
    );

    await t.test(
      "a gap below the retention floor requires a snapshot",
      async (t) => {
        const result = await recordReplay(
          checker,
          {
            ...expired,
            commands: [...expired.commands, install, ack, trim],
          },
          openWindow(t, null),
        );
        assert.equal(result.verdict, "matched", JSON.stringify(result));
        assert.deepEqual(result.recording.observations.at(-1), {
          cursor: "Three",
          floor: "Three",
          ack: "Three",
          buffered: [],
          retained: [],
          received: ["One", "Three", "Two"],
          gap: false,
        });
      },
    );

    await t.test(
      "acknowledging the largest received record prunes the missing record",
      async (t) => {
        const broken = openWindow(t, "maximum-as-prefix");
        const result = await recordReplay(
          checker,
          {
            ...hole,
            commands: [...hole.commands, ack, trim],
          },
          broken,
        );
        assert.equal(result.verdict, "mismatch");
        assert.deepEqual(result.difference.command, ack);
        assert.equal(result.difference.step, hole.commands.length + 1);
        await broken.apply(trim);
        await broken.apply(pull);
        assert.equal((await broken.observe()).gap, true);
        assert.deepEqual((await broken.observe()).retained, []);
      },
    );
  },
);
