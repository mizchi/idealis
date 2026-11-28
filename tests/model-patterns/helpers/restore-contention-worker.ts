import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { parentPort, workerData } from "node:worker_threads";

const port = parentPort;
assert.ok(port);
const { path, worker, lostCheck } = workerData as {
  path: string;
  worker: string;
  lostCheck: boolean;
};
const database = new DatabaseSync(path);
database.exec("PRAGMA busy_timeout = 1000");
const readCounter = database.prepare("SELECT high FROM counter");
const readAssignment = database.prepare(
  "SELECT id FROM assignment WHERE worker = ?",
);
const saveAssignment = database.prepare(
  "INSERT INTO assignment (worker, id) VALUES (?, ?)",
);
const writeCounter = database.prepare(
  lostCheck
    ? "UPDATE counter SET high = ?"
    : "UPDATE counter SET high = ? WHERE high = ?",
);

let proposal: { observed: number; id: number } | null = null;
port.on("message", (command: "reserve" | "commit" | "publish") => {
  let rejected = false;
  let published: number | null = null;
  switch (command) {
    case "reserve": {
      const observed = readCounter.get()?.high;
      assert.ok(typeof observed === "number");
      proposal = { observed, id: observed + 1 };
      break;
    }
    case "commit": {
      assert.ok(proposal);
      database.exec("BEGIN IMMEDIATE");
      try {
        const { changes } = lostCheck
          ? writeCounter.run(proposal.id)
          : writeCounter.run(proposal.id, proposal.observed);
        rejected = changes === 0;
        if (!rejected) saveAssignment.run(worker, proposal.id);
        database.exec("COMMIT");
        proposal = null;
      } catch (error) {
        database.exec("ROLLBACK");
        throw error;
      }
      break;
    }
    case "publish": {
      const id = readAssignment.get(worker)?.id;
      assert.ok(typeof id === "number");
      published = id;
      break;
    }
  }
  port.postMessage({ proposal: proposal?.id ?? null, rejected, published });
});
