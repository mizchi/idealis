import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = {
  readonly kind: "append" | "compact" | "pull" | "capture" | "install";
};
type Fault = "none" | "ignore-gap" | "watermark-from-live-source";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/lease/feed-compaction.model";

function sourceState(source: DatabaseSync) {
  const row = source
    .prepare("SELECT head, total, floor FROM source_state")
    .get();
  assert.ok(
    row &&
      typeof row.head === "number" &&
      typeof row.total === "number" &&
      typeof row.floor === "number",
  );
  return { head: row.head, total: row.total, floor: row.floor };
}

function replicaState(replica: DatabaseSync) {
  const row = replica
    .prepare("SELECT cursor, total, gap FROM replica_state")
    .get();
  assert.ok(
    row && typeof row.cursor === "number" && typeof row.total === "number",
  );
  return { cursor: row.cursor, total: row.total, gap: row.gap === 1 };
}

test("compacted change feed resnapshots at its captured position and handles a second gap", async (t) => {
  const base = await reachableScenario<Command>(
    checker,
    model,
    "NeedsInitialSnapshot",
  );
  assert.deepEqual(
    base.commands.map(({ kind }) => kind),
    ["append", "compact", "pull"],
  );

  for (const { compactAgain, fault } of [
    { compactAgain: false, fault: "none" },
    { compactAgain: true, fault: "none" },
    { compactAgain: false, fault: "ignore-gap" },
    { compactAgain: false, fault: "watermark-from-live-source" },
  ] as const satisfies readonly { compactAgain: boolean; fault: Fault }[]) {
    const dir = await mkdtemp(join(tmpdir(), "idealis-feed-compaction-"));
    t.after(() => rm(dir, { recursive: true, force: true }));
    using source = new DatabaseSync(join(dir, "source.sqlite"));
    using replica = new DatabaseSync(join(dir, "replica.sqlite"));
    source.exec(`
      CREATE TABLE source_state (head INTEGER, total INTEGER, floor INTEGER);
      INSERT INTO source_state VALUES (0, 0, 0);
      CREATE TABLE deltas (position INTEGER PRIMARY KEY, amount INTEGER NOT NULL);
    `);
    replica.exec(`
      CREATE TABLE replica_state (cursor INTEGER, total INTEGER, gap INTEGER);
      INSERT INTO replica_state VALUES (0, 0, 0);
    `);
    let snapshot: { head: number; total: number } | undefined;

    const tail: Command[] = [{ kind: "capture" }, { kind: "append" }];
    if (compactAgain) tail.push({ kind: "compact" });
    tail.push({ kind: "install" }, { kind: "pull" });
    if (compactAgain) tail.push({ kind: "capture" }, { kind: "install" });
    tail.push({ kind: "append" }, { kind: "pull" });

    const result = await recordReplay<Command>(
      checker,
      { ...base, commands: [...base.commands, ...tail] },
      {
        async apply({ kind }) {
          switch (kind) {
            case "append":
              source.exec(`
              BEGIN;
              UPDATE source_state SET head = head + 1, total = total + 2;
              INSERT INTO deltas SELECT head, 2 FROM source_state;
              COMMIT;
            `);
              break;
            case "compact":
              source.exec(`
              BEGIN;
              UPDATE source_state SET floor = head;
              DELETE FROM deltas WHERE position <= (SELECT floor FROM source_state);
              COMMIT;
            `);
              break;
            case "pull": {
              const { cursor } = replicaState(replica);
              const { floor } = sourceState(source);
              if (cursor < floor) {
                if (fault !== "ignore-gap")
                  replica.exec("UPDATE replica_state SET gap = 1");
                break;
              }
              const delta = source
                .prepare(
                  "SELECT position, amount FROM deltas WHERE position = ?",
                )
                .get(cursor + 1);
              if (!delta) {
                replica.exec("UPDATE replica_state SET gap = 0");
                break;
              }
              assert.ok(
                typeof delta.position === "number" &&
                  typeof delta.amount === "number",
              );
              replica
                .prepare(
                  "UPDATE replica_state SET cursor = ?, total = total + ?, gap = 0",
                )
                .run(delta.position, delta.amount);
              break;
            }
            case "capture":
              assert.equal(snapshot, undefined);
              snapshot = sourceState(source);
              break;
            case "install":
              assert.ok(snapshot);
              replica
                .prepare(
                  "UPDATE replica_state SET cursor = ?, total = ?, gap = 0",
                )
                .run(
                  fault === "watermark-from-live-source"
                    ? sourceState(source).head
                    : snapshot.head,
                  snapshot.total,
                );
              snapshot = undefined;
              break;
          }
        },
        async observe() {
          const { head, floor, total } = sourceState(source);
          const { cursor, total: replicaTotal, gap } = replicaState(replica);
          const { retained } =
            source.prepare("SELECT COUNT(*) AS retained FROM deltas").get() ??
            {};
          return {
            head,
            floor,
            sourceTotal: total,
            cursor,
            replicaTotal,
            gap,
            retained,
          };
        },
        project: ({ head, floor, sourceTotal, cursor, replicaTotal, gap }) => {
          assert.ok(typeof head === "number" && typeof floor === "number");
          return {
            head,
            floor,
            sourceTotal,
            cursor,
            replicaTotal,
            gap,
            retained: head - floor,
          };
        },
      },
    );

    if (fault !== "none") {
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, fault === "ignore-gap" ? 3 : 6);
      assert.deepEqual(
        result.difference.observed,
        fault === "ignore-gap"
          ? {
              head: 1,
              floor: 1,
              sourceTotal: 2,
              cursor: 0,
              replicaTotal: 0,
              gap: false,
              retained: 0,
            }
          : {
              head: 2,
              floor: 1,
              sourceTotal: 4,
              cursor: 2,
              replicaTotal: 2,
              gap: false,
              retained: 1,
            },
      );
    } else {
      assert.equal(result.verdict, "matched");
      if (compactAgain) {
        assert.deepEqual(result.recording.observations[8], {
          head: 2,
          floor: 2,
          sourceTotal: 4,
          cursor: 1,
          replicaTotal: 2,
          gap: true,
          retained: 0,
        });
      } else {
        assert.deepEqual(result.recording.observations[7], {
          head: 2,
          floor: 1,
          sourceTotal: 4,
          cursor: 2,
          replicaTotal: 4,
          gap: false,
          retained: 1,
        });
      }
      assert.deepEqual(result.recording.observations.at(-1), {
        head: 3,
        floor: compactAgain ? 2 : 1,
        sourceTotal: 6,
        cursor: 3,
        replicaTotal: 6,
        gap: false,
        retained: compactAgain ? 1 : 2,
      });
    }
  }
});
