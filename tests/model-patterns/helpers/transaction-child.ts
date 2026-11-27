import { DatabaseSync } from "node:sqlite";

const [path, mode] = process.argv.slice(2);
if (!path || !process.send) throw new Error("Expected a database path and IPC");
const db = new DatabaseSync(path);
process.on("message", (step) => {
  switch (step) {
    case "begin":
      db.exec("BEGIN");
      process.send?.({ phase: "OpenTxn" });
      break;
    case "stage":
      db.exec("INSERT INTO ledger DEFAULT VALUES");
      if (mode === "split") db.exec("COMMIT; BEGIN");
      process.send?.({ phase: "Prepared" });
      break;
    case "commit":
      db.exec("INSERT INTO receipt VALUES (1); COMMIT");
      process.send?.({ phase: "Committed" });
      break;
    case "respond": {
      const reply =
        db.prepare("SELECT id FROM receipt").get()?.id === 1 ? "Posted" : null;
      process.send?.({ phase: "Idle", reply });
      break;
    }
    default:
      throw new Error(`Unexpected transaction step ${String(step)}`);
  }
});
process.send({ phase: "Idle" });
