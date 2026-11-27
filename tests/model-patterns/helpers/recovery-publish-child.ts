import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";

export type Writer = "Original" | "Replacement";
export type Decision = "Stored" | "Rejected";
export type WorkerCommand = "prepare" | "publish";
export type WorkerState = { staged: boolean };

const [, , path, writer, fault] = process.argv;
assert.ok(path && (writer === "Original" || writer === "Replacement"));
const db = new DatabaseSync(path);
let staged = false;

function report() {
  process.send?.({ staged } satisfies WorkerState);
}

process.on("message", (command: WorkerCommand) => {
  switch (command) {
    case "prepare":
      assert.equal(
        db.prepare("SELECT owner FROM authority").get()?.owner,
        writer,
      );
      staged = true;
      break;
    case "publish": {
      assert.ok(staged);
      db.exec("BEGIN IMMEDIATE");
      try {
        const { changes } = db
          .prepare(
            "UPDATE visible SET author = ? WHERE ? OR (SELECT owner FROM authority) = ?",
          )
          .run(writer, Number(fault === "cached-grant"), writer);
        const result: Decision =
          changes || fault === "success-on-rejection" ? "Stored" : "Rejected";
        db.prepare("INSERT INTO receipts VALUES (?, ?)").run(writer, result);
        db.exec("COMMIT");
        staged = false;
      } catch (error) {
        db.exec("ROLLBACK");
        throw error;
      }
      break;
    }
  }
  report();
});

report();
