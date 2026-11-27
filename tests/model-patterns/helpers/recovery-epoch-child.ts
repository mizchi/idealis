import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";

export type Origin = "FirstProcess" | "SecondProcess";
export type Generation = "FirstBoot" | "SecondBoot";
export type Key = { generation: Generation; slot: "FirstSlot" };
export type Reply = { key: Key; origin: Origin };
export type WorkerState = { pending: Key | null; accepted: Origin | null };
export type WorkerCommand =
  | { kind: "request" }
  | { kind: "deliver"; reply: Reply };

const [, , path, fault] = process.argv;
assert.ok(path);
const db = new DatabaseSync(path);
const stored = db.prepare("SELECT epoch FROM recovery_generation").get()?.epoch;
const epoch =
  fault === "reuse-generation" && stored
    ? stored
    : db
        .prepare(
          "UPDATE recovery_generation SET epoch = epoch + 1 RETURNING epoch",
        )
        .get()?.epoch;
assert.ok(epoch === 1 || epoch === 2);
const generation: Generation = epoch === 1 ? "FirstBoot" : "SecondBoot";
db.close();

let pending: Key | null = null;
let accepted: Origin | null = null;
function report() {
  process.send?.({ pending, accepted } satisfies WorkerState);
}
process.on("message", (command: WorkerCommand) => {
  if (command.kind === "request") {
    pending = { generation, slot: "FirstSlot" };
  } else if (
    pending?.slot === command.reply.key.slot &&
    (fault === "slot-only" ||
      pending.generation === command.reply.key.generation)
  ) {
    accepted = command.reply.origin;
    pending = null;
  }
  report();
});
report();
