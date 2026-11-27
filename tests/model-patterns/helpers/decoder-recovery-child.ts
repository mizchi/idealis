import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";

export type Sequence = "First" | "Second";
export type Wire =
  | { kind: "seal"; last: number }
  | { kind: "receive"; sequence: Sequence; payload: number[] }
  | { kind: "ack"; sequence: Sequence }
  | { kind: "retire" };

const [, , path, fault] = process.argv;
assert.ok(path);
const db = new DatabaseSync(path);
const pendingAck = new Set<Sequence>();
const ephemeral = new Set<Sequence>();

function report() {
  process.send?.([...pendingAck].sort());
}

process.on("message", (message: Wire) => {
  switch (message.kind) {
    case "seal":
      db.prepare("UPDATE progress SET last = ?").run(message.last);
      break;
    case "ack":
      assert.ok(pendingAck.delete(message.sequence));
      break;
    case "retire":
      db.exec(`UPDATE progress SET retired = 1
        WHERE last IS NOT NULL AND last = (SELECT COUNT(*) FROM receipts)`);
      break;
    case "receive": {
      const { sequence, payload } = message;
      const retired =
        db.prepare("SELECT retired FROM progress").get()?.retired === 1;
      function recordDrop() {
        db.prepare("INSERT OR IGNORE INTO dropped VALUES (?)").run(sequence);
      }
      if (fault === "decode-first" && retired) {
        recordDrop();
        break;
      }
      const seen =
        fault === "volatile-dedup"
          ? ephemeral.has(sequence)
          : db
              .prepare("SELECT 1 FROM receipts WHERE sequence = ?")
              .get(sequence);
      if (!seen) {
        if (retired) {
          recordDrop();
          break;
        }
        assert.deepEqual(payload, [1, 1]);
        db.exec("BEGIN");
        try {
          db.prepare("INSERT INTO effects VALUES (?)").run(sequence);
          db.prepare("INSERT OR IGNORE INTO receipts VALUES (?)").run(sequence);
          db.exec("COMMIT");
        } catch (error) {
          db.exec("ROLLBACK");
          throw error;
        }
        ephemeral.add(sequence);
      }
      pendingAck.add(sequence);
      break;
    }
  }
  report();
});
report();
