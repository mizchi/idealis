import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Ticket = "Older" | "Current";
type Reply = { outcome: "Accepted" | "Expired"; atRevision: number };
type Command =
  | { readonly kind: "submit"; readonly ticket: Ticket }
  | { readonly kind: "collect"; readonly ticket: Ticket }
  | { readonly kind: "advanceFloor" | "reopen" };
type Fault = "none" | "accept-expired" | "collect-inclusive";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/offline-merge/retention.model";

test("receipt cleanup preserves retries at the boundary and rejects expired tickets after reopening", async (t) => {
  const expired = await reachableScenario<Command>(
    checker,
    model,
    "OldReceiptHasBeenCollected",
  );
  const atBoundary = await reachableScenario<Command>(
    checker,
    model,
    "NewTicketIsAtBoundary",
  );
  const older = { kind: "submit", ticket: "Older" } as const;
  const current = { kind: "submit", ticket: "Current" } as const;
  const cases = [
    {
      name: "after expiry",
      base: expired,
      tail: [{ kind: "reopen" }, older, current],
      fault: "accept-expired",
      mismatchStep: 5,
      expected: { revision: 2, reply: { outcome: "Accepted", atRevision: 2 } },
    },
    {
      name: "at the boundary",
      base: atBoundary,
      tail: [
        { kind: "collect", ticket: "Current" },
        { kind: "reopen" },
        current,
      ],
      fault: "collect-inclusive",
      mismatchStep: 5,
      expected: { revision: 1, reply: { outcome: "Accepted", atRevision: 1 } },
    },
  ] as const;
  for (const scenario of cases) {
    const faults: readonly Fault[] = ["none", scenario.fault];
    for (const fault of faults) {
      await t.test(`${scenario.name} / ${fault}`, async (t) => {
        const dir = await mkdtemp(join(tmpdir(), "idealis-retry-retention-"));
        const path = join(dir, "retention.sqlite");
        let db = new DatabaseSync(path);
        t.after(async () => {
          db.close();
          await rm(dir, { recursive: true, force: true });
        });
        db.exec(`
          CREATE TABLE admission (floor INTEGER NOT NULL);
          INSERT INTO admission VALUES (0);
          CREATE TABLE receipts (issued INTEGER, id TEXT, revision INTEGER, PRIMARY KEY (issued, id));
          CREATE TABLE edits (revision INTEGER PRIMARY KEY, issued INTEGER, id TEXT);
        `);
        const latest = () => {
          const row = db
            .prepare("SELECT COALESCE(MAX(revision), 0) AS revision FROM edits")
            .get();
          assert.ok(typeof row?.revision === "number");
          return row.revision;
        };
        const floor = () => {
          const row = db.prepare("SELECT floor FROM admission").get();
          assert.ok(typeof row?.floor === "number");
          return row.floor;
        };
        const issued = { Older: 0, Current: 1 } as const;
        const id = "document-edit";
        let reply: Reply | null = null;
        const commands: Command[] = [
          ...scenario.base.commands,
          ...scenario.tail,
        ];
        const result = await recordReplay<Command>(
          checker,
          { ...scenario.base, commands },
          {
            async apply(command) {
              switch (command.kind) {
                case "advanceFloor":
                  db.exec("UPDATE admission SET floor = 1");
                  break;
                case "collect":
                  db.prepare(
                    fault === "collect-inclusive"
                      ? "DELETE FROM receipts WHERE issued = ? AND issued <= ?"
                      : "DELETE FROM receipts WHERE issued = ? AND issued < ?",
                  ).run(issued[command.ticket], floor());
                  break;
                case "reopen":
                  db.close();
                  db = new DatabaseSync(path);
                  break;
                case "submit": {
                  const generation = issued[command.ticket];
                  db.exec("BEGIN IMMEDIATE");
                  try {
                    if (fault !== "accept-expired" && generation < floor()) {
                      reply = { outcome: "Expired", atRevision: latest() };
                    } else {
                      const receipt = db
                        .prepare(
                          "SELECT revision FROM receipts WHERE issued = ? AND id = ?",
                        )
                        .get(generation, id);
                      if (receipt) {
                        assert.ok(typeof receipt.revision === "number");
                        reply = {
                          outcome: "Accepted",
                          atRevision: receipt.revision,
                        };
                      } else {
                        const revision = latest() + 1;
                        db.prepare("INSERT INTO edits VALUES (?, ?, ?)").run(
                          revision,
                          generation,
                          id,
                        );
                        db.prepare("INSERT INTO receipts VALUES (?, ?, ?)").run(
                          generation,
                          id,
                          revision,
                        );
                        reply = { outcome: "Accepted", atRevision: revision };
                      }
                    }
                    db.exec("COMMIT");
                  } catch (error) {
                    db.exec("ROLLBACK");
                    throw error;
                  }
                  break;
                }
              }
            },
            async observe() {
              return { revision: latest(), reply };
            },
            project: ({ revision, reply }) => ({ revision, reply }),
          },
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(
            result.recording.observations.at(-1),
            scenario.expected,
          );
          if (scenario.base === expired) {
            assert.deepEqual(result.recording.observations[5], {
              revision: 1,
              reply: { outcome: "Expired", atRevision: 1 },
            });
            assert.equal(
              db.prepare("SELECT 1 FROM receipts WHERE issued = 0").get(),
              undefined,
            );
          }
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.deepEqual(result.difference, {
            step: scenario.mismatchStep,
            command: fault === "accept-expired" ? older : current,
            expected:
              fault === "accept-expired"
                ? { revision: 1, reply: { outcome: "Expired", atRevision: 1 } }
                : scenario.expected,
            observed: {
              revision: 2,
              reply: { outcome: "Accepted", atRevision: 2 },
            },
          });
        }
      });
    }
  }
});
