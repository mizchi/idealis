import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";

type CrashPoint = "BeforeCommit" | "AfterCommit";
type Command =
  | { readonly kind: "loseWorker"; readonly point: CrashPoint }
  | { readonly kind: "retry" };
type Fault =
  | "none"
  | "duplicate-effect"
  | "split-transaction"
  | "assume-failure";
type Outcome = "Pending" | "Unknown" | "Done" | "Failed";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/cancellation/worker-retry.model";

function runWorker(
  t: TestContext,
  path: string,
  mode: CrashPoint | "Retry",
  fault: Fault,
) {
  const worker = new Worker(
    new URL("./helpers/retry-worker.js", import.meta.url),
    {
      workerData: { path, mode, fault },
    },
  );
  t.after(() => worker.terminate());
  const messages: unknown[] = [];
  let error: Error | undefined;
  worker.on("message", (message: unknown) => messages.push(message));
  worker.on("error", (failure) => {
    error = failure;
  });
  return new Promise<{
    code: number;
    error: Error | undefined;
    messages: readonly unknown[];
  }>((resolve) => {
    worker.once("exit", (code) => resolve({ code, error, messages }));
  });
}

test(
  "a new worker recovers a receipt or applies the job after rollback",
  { timeout: 30_000 },
  async (t) => {
    const cases = [
      ["BeforeCommit", "none", null],
      ["AfterCommit", "none", null],
      ["AfterCommit", "duplicate-effect", 2],
      ["BeforeCommit", "split-transaction", 1],
      ["AfterCommit", "assume-failure", 1],
    ] as const;
    for (const [point, fault, mismatch] of cases) {
      await t.test(`${point} / ${fault}`, async (t) => {
        const scenario = await reachableScenario<Command>(
          checker,
          model,
          point === "BeforeCommit" ? "LostBeforeCommit" : "LostAfterCommit",
        );
        assert.deepEqual(scenario.commands, [{ kind: "loseWorker", point }]);
        const dir = await mkdtemp(join(tmpdir(), "idealis-worker-retry-"));
        t.after(() => rm(dir, { recursive: true, force: true }));
        const path = join(dir, "jobs.sqlite");
        const db = new DatabaseSync(path);
        try {
          db.exec(
            "CREATE TABLE ledger (job TEXT NOT NULL); CREATE TABLE receipts (job TEXT PRIMARY KEY, result TEXT NOT NULL)",
          );
        } finally {
          db.close();
        }
        let outcome: Outcome = "Pending";
        let reply: unknown = null;
        const adapter: ObservationAdapter<Command> = {
          async apply(command) {
            if (command.kind === "loseWorker") {
              const stopped = await runWorker(t, path, command.point, fault);
              assert.notEqual(stopped.code, 0);
              assert.equal(
                stopped.error?.message,
                `worker stopped at ${command.point}`,
              );
              assert.deepEqual(stopped.messages, []);
              outcome = fault === "assume-failure" ? "Failed" : "Unknown";
              return;
            }
            const stopped = await runWorker(t, path, "Retry", fault);
            assert.equal(stopped.error, undefined);
            assert.equal(stopped.code, 0);
            assert.equal(stopped.messages.length, 1);
            [reply] = stopped.messages;
            outcome = "Done";
          },
          async observe() {
            const store = new DatabaseSync(path, { readOnly: true });
            try {
              return {
                outcome,
                effects: store.prepare("SELECT count(*) AS n FROM ledger").get()
                  ?.n,
                receipt:
                  store.prepare("SELECT result FROM receipts").get()?.result ??
                  null,
                reply,
              };
            } finally {
              store.close();
            }
          },
          project: ({ outcome, effects, receipt, reply }) => ({
            outcome,
            effects,
            receipt,
            reply,
          }),
        };
        const result = await recordReplay(
          checker,
          {
            ...scenario,
            commands: [...scenario.commands, { kind: "retry" as const }],
          },
          adapter,
        );
        if (mismatch === null) {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations, [
            { outcome: "Pending", effects: 0, receipt: null, reply: null },
            {
              outcome: "Unknown",
              effects: point === "BeforeCommit" ? 0 : 1,
              receipt: point === "BeforeCommit" ? null : "Posted",
              reply: null,
            },
            { outcome: "Done", effects: 1, receipt: "Posted", reply: "Posted" },
          ]);
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, mismatch);
          const observed = result.difference.observed;
          if (fault === "duplicate-effect")
            assert.deepEqual(observed, {
              outcome: "Done",
              effects: 2,
              receipt: "Posted",
              reply: "Posted",
            });
          if (fault === "split-transaction")
            assert.deepEqual(observed, {
              outcome: "Unknown",
              effects: 1,
              receipt: null,
              reply: null,
            });
          if (fault === "assume-failure")
            assert.deepEqual(observed, {
              outcome: "Failed",
              effects: 1,
              receipt: "Posted",
              reply: null,
            });
        }
      });
    }
  },
);
