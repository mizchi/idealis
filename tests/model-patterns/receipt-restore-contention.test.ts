import assert from "node:assert/strict";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const workers = ["Left", "Right"] as const;
type Allocator = (typeof workers)[number];
type Command = { kind: "reserve" | "commit" | "publish"; worker: Allocator };
type Reply = {
  proposal: number | null;
  rejected: boolean;
  published: number | null;
};

const checker = "crates/rcheck/target/debug/rcheck";
const model =
  "examples/model-patterns/crash-recovery/receipt-restore-contention.model";

test("a stale allocator retries from the durable counter before publishing", async (t) => {
  const scenario = await reachableScenario<Command>(
    checker,
    model,
    "ContendersPublish",
  );
  assert.equal(scenario.commands.length, 8);
  assert.deepEqual(
    scenario.commands.slice(0, 2).map(({ kind }) => kind),
    ["reserve", "reserve"],
  );
  for (const lostCheck of [false, true]) {
    await t.test(
      lostCheck
        ? "unchecked commit duplicates the ID"
        : "atomic conditional commit",
      async (t) => {
        const root = await mkdtemp(
          join(tmpdir(), "idealis-restore-contention-"),
        );
        const path = join(root, "allocation.sqlite");
        using db = new DatabaseSync(path);
        db.exec(`
        CREATE TABLE counter (high INTEGER NOT NULL);
        INSERT INTO counter VALUES (0);
        CREATE TABLE assignment (worker TEXT PRIMARY KEY, id INTEGER NOT NULL);
      `);
        const sessions = new Map(
          workers.map(
            (worker) =>
              [
                worker,
                new Worker(
                  new URL(
                    "./helpers/restore-contention-worker.js",
                    import.meta.url,
                  ),
                  { workerData: { path, worker, lostCheck } },
                ),
              ] as const,
          ),
        );
        t.after(async () => {
          await Promise.all(
            [...sessions.values()].map((worker) => worker.terminate()),
          );
          await rm(root, { recursive: true, force: true });
        });
        const proposed: Record<Allocator, number | null> = {
          Left: null,
          Right: null,
        };
        const published: Record<Allocator, number | null> = {
          Left: null,
          Right: null,
        };
        const stale = new Set<Allocator>();
        const lookup = db.prepare("SELECT id FROM assignment WHERE worker = ?");
        const high = db.prepare("SELECT high FROM counter");
        const result = await recordReplay(checker, scenario, {
          async apply(command) {
            const worker = sessions.get(command.worker);
            assert.ok(worker);
            const response = once(worker, "message");
            worker.postMessage(command.kind);
            const [reply] = (await response) as [Reply];
            proposed[command.worker] = reply.proposal;
            if (reply.rejected) stale.add(command.worker);
            if (reply.published !== null)
              published[command.worker] = reply.published;
          },
          async observe() {
            return {
              high: high.get()?.high,
              proposed: { ...proposed },
              assigned: {
                Left: lookup.get("Left")?.id ?? null,
                Right: lookup.get("Right")?.id ?? null,
              },
              published: { ...published },
              stale: workers.filter((worker) => stale.has(worker)),
            };
          },
          project: (state) => state,
        });
        if (!lostCheck) {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(published, { Left: 1, Right: 2 });
          return;
        }
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, 4);
        assert.deepEqual(result.difference.command, {
          kind: "commit",
          worker: "Right",
        });
        const expected = result.difference.expected as {
          assigned: Record<Allocator, number | null>;
        };
        const actual = result.difference.observed as typeof expected;
        assert.deepEqual(expected.assigned, { Left: 1, Right: null });
        assert.deepEqual(actual.assigned, { Left: 1, Right: 1 });
      },
    );
  }
});
