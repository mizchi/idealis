import { DatabaseSync } from "node:sqlite";
import { parentPort, workerData } from "node:worker_threads";

const { path, mode, fault } = workerData as {
  path: string;
  mode: "BeforeCommit" | "AfterCommit" | "Retry";
  fault: "none" | "duplicate-effect" | "split-transaction" | "assume-failure";
};
if (!parentPort) throw new Error("Worker thread required");

const db = new DatabaseSync(path);
const job = "stable-job";
const findReceipt = db.prepare("SELECT result FROM receipts WHERE job = ?");
db.exec("BEGIN IMMEDIATE");
const saved = findReceipt.get(job);
if (saved === undefined || fault === "duplicate-effect") {
  db.prepare("INSERT INTO ledger VALUES (?)").run(job);
}
if (fault === "split-transaction") db.exec("COMMIT; BEGIN IMMEDIATE");
if (saved === undefined) {
  db.prepare("INSERT INTO receipts VALUES (?, ?)").run(job, "Posted");
}
if (mode === "BeforeCommit") throw new Error("worker stopped at BeforeCommit");
db.exec("COMMIT");
const result = findReceipt.get(job)?.result;
db.close();
if (mode === "AfterCommit") throw new Error("worker stopped at AfterCommit");
parentPort.postMessage(result);
