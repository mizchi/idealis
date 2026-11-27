import { DatabaseSync } from "node:sqlite";
import { parentPort, workerData } from "node:worker_threads";

type Attempt = "Original" | "Replacement";

const { path, job, attempt, acceptOldWrite } = workerData as {
  path: string;
  job: string;
  attempt: Attempt;
  acceptOldWrite: boolean;
};
const port = parentPort;
if (!port) throw new Error("Worker thread required");

port.once("message", (command) => {
  if (command !== "write") throw new Error(`unexpected command: ${command}`);
  const db = new DatabaseSync(path);
  try {
    db.exec("BEGIN IMMEDIATE");
    const owner = db
      .prepare("SELECT attempt FROM assignments WHERE job = ?")
      .get(job)?.attempt;
    if (owner !== attempt && !acceptOldWrite) {
      db.exec("ROLLBACK");
      port.postMessage({ status: "stale", attempt });
      return;
    }
    const saved = db
      .prepare("SELECT attempt FROM receipts WHERE job = ?")
      .get(job)?.attempt;
    if (saved === undefined) {
      db.prepare("INSERT INTO ledger VALUES (?, ?)").run(job, attempt);
      db.prepare("INSERT INTO receipts VALUES (?, ?)").run(job, attempt);
    }
    db.exec("COMMIT");
    port.postMessage({ status: "ok", attempt, receipt: saved ?? attempt });
  } finally {
    db.close();
  }
});
port.postMessage({ phase: "ready", attempt });
