import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import { replay, type Scenario } from "../../examples/model-patterns/replay.js";

const ids = ["First", "Second"] as const;
type Command = {
  readonly kind: "deliver" | "loseResponse";
  readonly request: (typeof ids)[number];
};
const scenario: Scenario<Command> = {
  model: "examples/model-patterns/retry/delivery.model",
  config: {},
  initial: {
    effects: { First: null, Second: null },
    receipts: [],
    replies: [],
  },
  commands: [
    { kind: "deliver", request: "First" },
    { kind: "loseResponse", request: "First" },
    { kind: "deliver", request: "Second" },
    { kind: "deliver", request: "First" },
  ],
};

test("retry after losing a committed response reads the receipt from a reopened database", async () => {
  const dir = await mkdtemp(join(tmpdir(), "idealis-retry-"));
  try {
    for (const applyDuplicate of [false, true]) {
      const path = join(
        dir,
        applyDuplicate ? "faulty.sqlite" : "correct.sqlite",
      );
      let db = new DatabaseSync(path);
      try {
        db.exec(`
          CREATE TABLE receipts (request TEXT PRIMARY KEY);
          CREATE TABLE ledger (request TEXT NOT NULL);
        `);
        const replies = new Set<string>();
        const run = replay("crates/rcheck/target/debug/rcheck", scenario, {
          async apply(command) {
            if (command.kind === "loseResponse") {
              replies.delete(command.request);
              db.close();
              db = new DatabaseSync(path);
              return;
            }
            db.exec("BEGIN");
            try {
              const receipt = db
                .prepare("INSERT OR IGNORE INTO receipts VALUES (?)")
                .run(command.request);
              if (Number(receipt.changes) === 1 || applyDuplicate) {
                db.prepare("INSERT INTO ledger VALUES (?)").run(
                  command.request,
                );
              }
              db.exec("COMMIT");
            } catch (error) {
              db.exec("ROLLBACK");
              throw error;
            }
            const row = db
              .prepare("SELECT request FROM receipts WHERE request = ?")
              .get(command.request);
            assert.equal(row?.request, command.request);
            replies.add(command.request);
          },
          async observe() {
            const effects = Object.fromEntries(
              ids.map((id) => {
                const count = db
                  .prepare("SELECT COUNT(*) AS n FROM ledger WHERE request = ?")
                  .get(id)?.n;
                return [
                  id,
                  count === 0 ? null : count === 1 ? "Posted" : "Duplicate",
                ];
              }),
            );
            return {
              effects,
              receipts: db
                .prepare("SELECT request FROM receipts ORDER BY request")
                .all()
                .map((row) => row.request),
              replies: [...replies].sort(),
            };
          },
          project(state) {
            return {
              effects: state.effects,
              receipts: [...(state.receipts as string[])].sort(),
              replies: [...(state.replies as string[])].sort(),
            };
          },
        });
        if (applyDuplicate) {
          await assert.rejects(
            run,
            /"step":4.*"expected":\{"effects":\{"First":"Posted".*"observed":\{"effects":\{"First":"Duplicate"/,
          );
        } else {
          await run;
        }
      } finally {
        db.close();
      }
    }
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
