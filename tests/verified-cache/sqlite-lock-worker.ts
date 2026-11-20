import { DatabaseSync } from "node:sqlite";
import { parentPort, workerData } from "node:worker_threads";

const port = parentPort;
if (!port) throw new Error("Expected a worker thread");

const db = new DatabaseSync(workerData);
db.exec("BEGIN EXCLUSIVE");
port.postMessage("locked");
port.once("message", () => {
  setTimeout(() => {
    db.exec("COMMIT");
    db.close();
    port.close();
  }, 150);
});
