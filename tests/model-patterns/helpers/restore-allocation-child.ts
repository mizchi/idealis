import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";

export type Ticket = "First" | "Second";
export type Phase = "Idle" | "Preparing" | "Durable" | "Waiting" | "Finished";
export type Envelope = { ticket: Ticket; number: number };
export type WorkerCommand =
  | { kind: "reserve"; ticket: Ticket }
  | { kind: "commit" | "send" }
  | { kind: "deliver"; envelope: Envelope };
export type WorkerState = {
  phase: Phase;
  ticket: Ticket | null;
  number: number | null;
  accepted: Ticket | null;
  emitted: Envelope | null;
};

const [, , path, fault] = process.argv;
assert.ok(path);
const db = new DatabaseSync(path);
let phase: Phase = "Idle";
let reserved: Envelope | null = null;
let accepted: Ticket | null = null;
let volatileCounter = 0;

function report(emitted: Envelope | null = null) {
  process.send?.({
    phase,
    ticket: reserved?.ticket ?? null,
    number: reserved?.number ?? null,
    accepted,
    emitted,
  } satisfies WorkerState);
}

process.on("message", (command: WorkerCommand) => {
  switch (command.kind) {
    case "reserve": {
      assert.equal(phase, "Idle");
      db.exec("BEGIN IMMEDIATE");
      const row = db.prepare("SELECT high FROM allocation").get();
      assert.ok(row);
      const number =
        (fault === "volatile-counter" ? volatileCounter : Number(row.high)) + 1;
      if (fault !== "volatile-counter")
        db.prepare("UPDATE allocation SET high = ?").run(number);
      reserved = { ticket: command.ticket, number };
      phase = "Preparing";
      report(fault === "publish-early" ? reserved : null);
      return;
    }
    case "commit":
      assert.equal(phase, "Preparing");
      assert.ok(reserved);
      db.exec("COMMIT");
      volatileCounter = reserved.number;
      phase = "Durable";
      break;
    case "send":
      assert.equal(phase, "Durable");
      assert.ok(reserved);
      phase = "Waiting";
      report(reserved);
      return;
    case "deliver":
      if (phase === "Waiting" && command.envelope.number === reserved?.number) {
        accepted = command.envelope.ticket;
        phase = "Finished";
      }
      break;
  }
  report();
});
report();
