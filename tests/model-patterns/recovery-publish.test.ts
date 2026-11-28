import assert from "node:assert/strict";
import { fork, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import { fileURLToPath } from "node:url";
import {
  recordReplay,
  transitionScenario,
} from "../../examples/model-patterns/replay.js";
import type {
  Decision,
  WorkerCommand,
  WorkerState,
  Writer,
} from "./helpers/recovery-publish-child.js";

type Command = { kind: "promote" } | { kind: WorkerCommand; writer: Writer };
type Fault = "cached-grant" | "success-on-rejection";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/crash-recovery/recovery-publish.ideal";
const writers = ["Original", "Replacement"] as const;

async function report(child: ChildProcess, command?: WorkerCommand) {
  const response = once(child, "message", {
    signal: AbortSignal.timeout(5_000),
  });
  if (command) child.send(command);
  const [state] = (await response) as [WorkerState];
  return state.staged;
}

async function openRecovery(t: TestContext, fault?: Fault) {
  const root = await mkdtemp(join(tmpdir(), "idealis-recovery-publish-"));
  const path = join(root, "view.sqlite");
  const db = new DatabaseSync(path);
  const workers = new Map<Writer, { child: ChildProcess; staged: boolean }>();
  t.after(async () => {
    await Promise.all(
      [...workers.values()].map(async ({ child }) => {
        if (child.exitCode !== null || child.signalCode !== null) return;
        const exited = once(child, "exit");
        child.kill();
        await exited;
      }),
    );
    db.close();
    await rm(root, { recursive: true, force: true });
  });
  db.exec(`
    CREATE TABLE authority (owner TEXT NOT NULL);
    INSERT INTO authority VALUES ('Original');
    CREATE TABLE visible (author TEXT);
    INSERT INTO visible VALUES (NULL);
    CREATE TABLE receipts (writer TEXT PRIMARY KEY, decision TEXT NOT NULL);
  `);

  async function launch(writer: Writer) {
    const child = fork(
      fileURLToPath(
        new URL("./helpers/recovery-publish-child.js", import.meta.url),
      ),
      [path, writer, fault ?? "normal"],
      { stdio: ["ignore", "ignore", "inherit", "ipc"] },
    );
    const worker = { child, staged: false };
    workers.set(writer, worker);
    worker.staged = await report(child);
  }

  await launch("Original");
  return {
    async apply(command: Command) {
      if (command.kind === "promote") {
        db.prepare("UPDATE authority SET owner = 'Replacement'").run();
        await launch("Replacement");
      } else {
        const worker = workers.get(command.writer);
        assert.ok(worker);
        worker.staged = await report(worker.child, command.kind);
      }
    },
    async observe() {
      const receipt = db.prepare(
        "SELECT decision FROM receipts WHERE writer = ?",
      );
      return {
        owner: db.prepare("SELECT owner FROM authority").get()?.owner as Writer,
        published: db.prepare("SELECT author FROM visible").get()
          ?.author as Writer | null,
        staged: writers.filter((writer) => workers.get(writer)?.staged),
        receipts: Object.fromEntries(
          writers.map((writer) => [
            writer,
            (receipt.get(writer)?.decision ?? null) as Decision | null,
          ]),
        ),
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "a prepared result must still hold the durable publication grant",
  { timeout: 30_000 },
  async (t) => {
    for (const prior of ["none", "Replacement"] as const) {
      const scenario = await transitionScenario<Command>(
        checker,
        model,
        "publish",
        `writer = Original and before(owner) = Replacement and
       before(published) = ${prior} and after(Original.receipts) = Rejected`,
      );
      assert.deepEqual(scenario.commands.at(-1), {
        kind: "publish",
        writer: "Original",
      });
      await t.test(`normal with visible ${prior}`, async (t) => {
        const commands: Command[] = [...scenario.commands];
        if (prior === "none") {
          commands.push(
            { kind: "prepare", writer: "Replacement" },
            { kind: "publish", writer: "Replacement" },
          );
        }
        const run = await recordReplay(
          checker,
          { ...scenario, commands },
          await openRecovery(t),
        );
        assert.equal(run.verdict, "matched");
        assert.deepEqual(run.recording.observations[scenario.commands.length], {
          owner: "Replacement",
          published: prior === "none" ? null : "Replacement",
          staged: [],
          receipts: {
            Original: "Rejected",
            Replacement: prior === "none" ? null : "Stored",
          },
        });
        assert.deepEqual(run.recording.observations.at(-1), {
          owner: "Replacement",
          published: "Replacement",
          staged: [],
          receipts: { Original: "Rejected", Replacement: "Stored" },
        });
      });
      for (const fault of ["cached-grant", "success-on-rejection"] as const) {
        await t.test(`${fault} with visible ${prior}`, async (t) => {
          const run = await recordReplay(
            checker,
            scenario,
            await openRecovery(t, fault),
          );
          assert.equal(run.verdict, "mismatch");
          assert.equal(run.difference.step, scenario.commands.length);
          const observed = run.difference.observed as {
            published: Writer | null;
            receipts: Record<Writer, Decision | null>;
          };
          assert.equal(observed.receipts.Original, "Stored");
          assert.equal(
            observed.published,
            fault === "cached-grant"
              ? "Original"
              : prior === "none"
                ? null
                : "Replacement",
          );
        });
      }
    }
  },
);
