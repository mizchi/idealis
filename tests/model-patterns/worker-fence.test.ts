import assert from "node:assert/strict";
import { once } from "node:events";
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

type Attempt = "Original" | "Replacement";
type Command =
  | { readonly kind: "replace" }
  | { readonly kind: "write" | "deliver"; readonly attempt: Attempt };
type Fault = "none" | "accept-old-write" | "accept-old-reply";
type Completion =
  | { readonly status: "stale"; readonly attempt: Attempt }
  | {
      readonly status: "ok";
      readonly attempt: Attempt;
      readonly receipt: Attempt;
    };

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/cancellation/worker-fence.model";
const job = "job-with-retry";

async function startWorker(
  t: TestContext,
  path: string,
  attempt: Attempt,
  fault: Fault,
) {
  const worker = new Worker(
    new URL("./helpers/fenced-worker.js", import.meta.url),
    {
      workerData: {
        path,
        job,
        attempt,
        acceptOldWrite: fault === "accept-old-write",
      },
    },
  );
  t.after(() => worker.terminate());
  const [message] = await once(worker, "message");
  assert.deepEqual(message, { phase: "ready", attempt });
  return worker;
}

test(
  "a timed-out worker cannot write or deliver a stale success",
  { timeout: 30_000 },
  async (t) => {
    const cases = [
      ["old-write", "none"],
      ["old-write", "accept-old-write"],
      ["old-reply", "none"],
      ["old-reply", "accept-old-reply"],
    ] as const;
    for (const [race, fault] of cases) {
      await t.test(`${race} / ${fault}`, async (t) => {
        const scenario = await reachableScenario<Command>(
          checker,
          model,
          race === "old-write"
            ? "ReplacementBeforeAnyCommit"
            : "OldCommitAwaitingReply",
        );
        const commands: Command[] = [...scenario.commands];
        if (race === "old-write") {
          assert.deepEqual(commands, [{ kind: "replace" }]);
          commands.push({ kind: "write", attempt: "Original" });
        } else {
          assert.deepEqual(commands, [
            { kind: "write", attempt: "Original" },
            { kind: "replace" },
          ]);
        }
        commands.push(
          { kind: "write", attempt: "Replacement" },
          { kind: "deliver", attempt: "Replacement" },
        );
        if (race === "old-reply")
          commands.push({ kind: "deliver", attempt: "Original" });

        const dir = await mkdtemp(join(tmpdir(), "idealis-worker-fence-"));
        t.after(() => rm(dir, { recursive: true, force: true }));
        const path = join(dir, "job.sqlite");
        const db = new DatabaseSync(path);
        t.after(() => db.close());
        db.exec(`
        CREATE TABLE assignments (job TEXT PRIMARY KEY, attempt TEXT NOT NULL);
        CREATE TABLE ledger (job TEXT NOT NULL, attempt TEXT NOT NULL);
        CREATE TABLE receipts (job TEXT PRIMARY KEY, attempt TEXT NOT NULL);
      `);
        db.prepare("INSERT INTO assignments VALUES (?, ?)").run(
          job,
          "Original",
        );

        const workers = new Map<Attempt, Worker>([
          ["Original", await startWorker(t, path, "Original", fault)],
        ]);
        const completed = new Map<Attempt, Completion>();
        let current: Attempt = "Original";
        let replyFrom: Attempt | null = null;
        let replyReceipt: Attempt | null = null;

        const adapter = {
          async apply(command) {
            if (command.kind === "replace") {
              db.prepare(
                "UPDATE assignments SET attempt = ? WHERE job = ?",
              ).run("Replacement", job);
              current = "Replacement";
              workers.set(current, await startWorker(t, path, current, fault));
              return;
            }
            const { attempt } = command;
            if (command.kind === "write") {
              const worker = workers.get(attempt);
              assert.ok(worker);
              const response = once(worker, "message");
              worker.postMessage("write");
              const [message] = await response;
              assert.equal(message.attempt, attempt);
              assert.ok(message.status === "ok" || message.status === "stale");
              if (message.status === "ok")
                assert.ok(
                  message.receipt === "Original" ||
                    message.receipt === "Replacement",
                );
              completed.set(attempt, message);
              return;
            }
            const response = completed.get(attempt);
            assert.equal(response?.status, "ok");
            if (response.attempt === current || fault === "accept-old-reply") {
              replyFrom = response.attempt;
              replyReceipt = response.receipt;
            }
          },
          async observe() {
            return {
              current,
              completed: [...completed.keys()].sort(),
              ready: [...completed]
                .filter(([, reply]) => reply.status === "ok")
                .map(([attempt]) => attempt)
                .sort(),
              effects: db
                .prepare(
                  "SELECT attempt FROM ledger WHERE job = ? ORDER BY attempt",
                )
                .all(job)
                .map((row) => row.attempt),
              receipt:
                db
                  .prepare("SELECT attempt FROM receipts WHERE job = ?")
                  .get(job)?.attempt ?? null,
              replyFrom,
              replyReceipt,
            };
          },
          project: ({
            current,
            completed,
            ready,
            effects,
            receipt,
            replyFrom,
            replyReceipt,
          }) => ({
            current,
            completed,
            ready,
            effects,
            receipt,
            replyFrom,
            replyReceipt,
          }),
        } satisfies ObservationAdapter<Command>;

        const result = await recordReplay(
          checker,
          { ...scenario, commands },
          adapter,
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          const committedBy = race === "old-write" ? "Replacement" : "Original";
          assert.deepEqual(result.recording.observations.at(-1), {
            current: "Replacement",
            completed: ["Original", "Replacement"],
            ready:
              race === "old-write"
                ? ["Replacement"]
                : ["Original", "Replacement"],
            effects: [committedBy],
            receipt: committedBy,
            replyFrom: "Replacement",
            replyReceipt: committedBy,
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(
            result.difference.step,
            fault === "accept-old-write" ? 2 : 5,
          );
          assert.deepEqual(
            result.difference.observed,
            fault === "accept-old-write"
              ? {
                  current: "Replacement",
                  completed: ["Original"],
                  ready: ["Original"],
                  effects: ["Original"],
                  receipt: "Original",
                  replyFrom: null,
                  replyReceipt: null,
                }
              : {
                  current: "Replacement",
                  completed: ["Original", "Replacement"],
                  ready: ["Original", "Replacement"],
                  effects: ["Original"],
                  receipt: "Original",
                  replyFrom: "Original",
                  replyReceipt: "Original",
                },
          );
        }
      });
    }
  },
);
