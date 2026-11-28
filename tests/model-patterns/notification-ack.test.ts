import assert from "node:assert/strict";
import { once } from "node:events";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import { MessageChannel } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Notice = "Primary" | "Other";
type Generation = "Old" | "New";
type Command = {
  kind: "publish" | "deliver" | "ack";
  notice: Notice;
  generation: Generation;
};

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/ack.model";
const ack = (notice: Notice, generation: Generation): Command => ({
  kind: "ack",
  notice,
  generation,
});

test("a delayed acknowledgement only settles the exact published generation", async () => {
  const reused = await reachableScenario<Command>(
    checker,
    model,
    "ReusedWithOldAck",
  );
  const parallel = await reachableScenario<Command>(
    checker,
    model,
    "TwoIdsPending",
  );
  assert.equal(reused.commands.length, 4);
  assert.equal(parallel.commands.length, 3);
  for (const [base, suffix, fault] of [
    [
      reused,
      [
        ack("Primary", "Old"),
        { kind: "deliver", notice: "Primary", generation: "New" },
        ack("Primary", "New"),
      ],
      "none",
    ],
    [reused, [ack("Primary", "Old")], "id-only"],
    [parallel, [ack("Primary", "Old")], "none"],
    [parallel, [ack("Primary", "Old")], "generation-only"],
  ] as const) {
    const { port1, port2 } = new MessageChannel();
    const db = new DatabaseSync(":memory:");
    db.exec(`CREATE TABLE pending (notice TEXT PRIMARY KEY, generation TEXT NOT NULL);
      CREATE TABLE settled (notice TEXT NOT NULL, generation TEXT NOT NULL, PRIMARY KEY (notice, generation))`);
    const observed: Command[] = [];
    function pairRows(table: "pending" | "settled") {
      return db
        .prepare(`SELECT notice, generation FROM ${table} ORDER BY generation`)
        .all();
    }
    try {
      const result = await recordReplay<Command>(
        checker,
        { ...base, commands: [...base.commands, ...suffix] },
        {
          async apply(command) {
            if (command.kind === "publish") {
              db.prepare("INSERT INTO pending VALUES (?, ?)").run(
                command.notice,
                command.generation,
              );
            } else if (command.kind === "deliver") {
              const arrived = once(port2, "message");
              port1.postMessage(command);
              const [received] = await arrived;
              observed.push(received as Command);
            } else {
              const delivered = observed.find(
                (item) =>
                  item.notice === command.notice &&
                  item.generation === command.generation,
              );
              assert.ok(delivered);
              const arrived = once(port1, "message");
              port2.postMessage({
                notice: delivered.notice,
                generation: delivered.generation,
              });
              const [payload] = await arrived;
              const { notice, generation } = payload as Pick<
                Command,
                "notice" | "generation"
              >;
              db.exec("BEGIN");
              try {
                const selector =
                  fault === "id-only"
                    ? { predicate: "notice = :notice", bindings: { notice } }
                    : fault === "generation-only"
                      ? {
                          predicate: "generation = :generation",
                          bindings: { generation },
                        }
                      : {
                          predicate:
                            "notice = :notice AND generation = :generation",
                          bindings: { notice, generation },
                        };
                const removed = db
                  .prepare(
                    `DELETE FROM pending WHERE ${selector.predicate} RETURNING notice, generation`,
                  )
                  .all(selector.bindings);
                for (const row of removed) {
                  assert.ok(typeof row.notice === "string");
                  assert.ok(typeof row.generation === "string");
                  db.prepare("INSERT INTO settled VALUES (?, ?)").run(
                    row.notice,
                    row.generation,
                  );
                }
                db.exec("COMMIT");
              } catch (error) {
                db.exec("ROLLBACK");
                throw error;
              }
            }
          },
          async observe() {
            const pending: Record<Notice, Generation | null> = {
              Primary: null,
              Other: null,
            };
            const settled: Record<Notice, Generation[]> = {
              Primary: [],
              Other: [],
            };
            for (const row of pairRows("pending"))
              pending[row.notice as Notice] = row.generation as Generation;
            for (const row of pairRows("settled"))
              settled[row.notice as Notice].push(row.generation as Generation);
            return { pending, settled };
          },
          project: ({ pending, settled }) => ({
            pending,
            settled: Object.fromEntries(
              Object.entries(settled as Record<Notice, Generation[]>).map(
                ([notice, values]) => [notice, [...values].sort()],
              ),
            ),
          }),
        },
      );
      if (fault === "none") {
        assert.equal(result.verdict, "matched");
        assert.deepEqual(
          result.recording.observations.at(-1),
          base === reused
            ? {
                pending: { Primary: null, Other: null },
                settled: { Primary: ["New", "Old"], Other: [] },
              }
            : {
                pending: { Primary: null, Other: "Old" },
                settled: { Primary: ["Old"], Other: [] },
              },
        );
      } else {
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, base.commands.length + 1);
        assert.deepEqual(
          result.difference.observed,
          fault === "id-only"
            ? {
                pending: { Primary: null, Other: null },
                settled: { Primary: ["New", "Old"], Other: [] },
              }
            : {
                pending: { Primary: null, Other: null },
                settled: { Primary: ["Old"], Other: ["Old"] },
              },
        );
      }
    } finally {
      port1.close();
      port2.close();
      db.close();
    }
  }
});
