import { readFile } from "node:fs/promises";
import { createSqliteCheckCacheStore } from "../../../packages/idealis/src/check-cache-node.js";
import { createCheckCache } from "../../../packages/idealis/src/check-cache.js";

const [database, input] = process.argv.slice(2);
if (!database || !input) throw new Error("Expected database and job file");
const store = createSqliteCheckCacheStore(database);
const cache = createCheckCache(store);
const reservation = await cache.acquire(
  JSON.parse(await readFile(input, "utf8")),
  { owner: "crashed-worker", runId: "crash", now: 0, leaseMs: 2 },
);
if (reservation.kind !== "lease")
  throw new Error("Worker did not acquire lease");
console.log(JSON.stringify(reservation.lease));
process.stdin.resume();
