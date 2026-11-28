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
  Generation,
  Origin,
  Reply,
  WorkerCommand,
  WorkerState,
} from "./helpers/recovery-epoch-child.js";

type Command =
  | { kind: "request" | "crash" | "restart" }
  | { kind: "deliver"; origin: Origin };
type Fault = "slot-only" | "reuse-generation";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/recovery-epoch.model";

async function report(child: ChildProcess, command?: WorkerCommand) {
  const response = once(child, "message", {
    signal: AbortSignal.timeout(5_000),
  });
  if (command) child.send(command);
  const [state] = (await response) as [WorkerState];
  return state;
}

async function kill(child: ChildProcess) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, "exit");
  assert.equal(child.kill("SIGKILL"), true);
  assert.deepEqual(await exited, [null, "SIGKILL"]);
}

async function openRecovery(t: TestContext, fault?: Fault) {
  const root = await mkdtemp(join(tmpdir(), "idealis-recovery-epoch-"));
  const path = join(root, "recovery.sqlite");
  const db = new DatabaseSync(path);
  let child: ChildProcess | undefined;
  t.after(async () => {
    if (child) await kill(child);
    db.close();
    await rm(root, { recursive: true, force: true });
  });
  db.exec(`
    CREATE TABLE recovery_generation (epoch INTEGER NOT NULL);
    INSERT INTO recovery_generation VALUES (0);
  `);
  const archived = new Map<Origin, Reply>();
  const consumed = new Set<Origin>();
  const launch = () =>
    fork(
      fileURLToPath(
        new URL("./helpers/recovery-epoch-child.js", import.meta.url),
      ),
      [path, fault ?? "normal"],
      { stdio: ["ignore", "ignore", "inherit", "ipc"] },
    );

  let active: Origin | null = "FirstProcess";
  child = launch();
  let state = await report(child);

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "request":
          assert.ok(child && active);
          state = await report(child, { kind: "request" });
          assert.ok(state.pending);
          archived.set(active, { key: state.pending, origin: active });
          break;
        case "crash":
          assert.ok(child);
          await kill(child);
          child = undefined;
          active = null;
          state = { pending: null, accepted: null };
          break;
        case "restart":
          child = launch();
          state = await report(child);
          active = "SecondProcess";
          break;
        case "deliver": {
          assert.ok(child);
          const reply = archived.get(command.origin);
          assert.ok(reply);
          state = await report(child, { kind: "deliver", reply });
          consumed.add(command.origin);
          break;
        }
      }
    },
    async observe() {
      const epoch = db
        .prepare("SELECT epoch FROM recovery_generation")
        .get()?.epoch;
      assert.ok(epoch === 1 || epoch === 2);
      const generation: Generation = epoch === 1 ? "FirstBoot" : "SecondBoot";
      return {
        active,
        generation,
        broker: {
          FirstProcess: archived.get("FirstProcess")?.key ?? null,
          SecondProcess: archived.get("SecondProcess")?.key ?? null,
        },
        consumed: [...consumed].sort(),
        ...state,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "a response queued before a process crash cannot resolve its replacement's request",
  { timeout: 30_000 },
  async (t) => {
    const scenario = await transitionScenario<Command>(
      checker,
      model,
      "deliver",
      "origin = FirstProcess and before(active) = SecondProcess and some before(pending) and unchanged(accepted)",
    );
    assert.deepEqual(
      scenario.commands.map((command) => command.kind),
      ["request", "crash", "restart", "request", "deliver"],
    );

    await t.test(
      "the next process ignores the archived response and accepts its own",
      async (t) => {
        const run = await recordReplay(
          checker,
          {
            ...scenario,
            commands: [
              ...scenario.commands,
              { kind: "deliver", origin: "SecondProcess" },
            ],
          },
          await openRecovery(t),
        );
        assert.equal(run.verdict, "matched");
        const afterOld = run.recording.observations.at(-2) as WorkerState;
        assert.deepEqual(afterOld.pending, {
          generation: "SecondBoot",
          slot: "FirstSlot",
        });
        assert.equal(afterOld.accepted, null);
        const afterOwn = run.recording.observations.at(-1) as WorkerState;
        assert.equal(afterOwn.accepted, "SecondProcess");
        assert.equal(afterOwn.pending, null);
      },
    );

    await t.test(
      "matching only the volatile slot accepts the crashed process's reply",
      async (t) => {
        const run = await recordReplay(
          checker,
          scenario,
          await openRecovery(t, "slot-only"),
        );
        assert.equal(run.verdict, "mismatch");
        assert.equal(run.difference.step, scenario.commands.length);
        assert.equal(
          (run.difference.observed as WorkerState).accepted,
          "FirstProcess",
        );
      },
    );

    await t.test(
      "reusing the stored generation defeats the full key check",
      async (t) => {
        const adapter = await openRecovery(t, "reuse-generation");
        const run = await recordReplay(checker, scenario, adapter);
        assert.equal(run.verdict, "mismatch");
        assert.deepEqual(run.difference.command, { kind: "restart" });
        for (const command of scenario.commands.slice(run.difference.step)) {
          await adapter.apply(command);
        }
        const actual = await adapter.observe();
        assert.deepEqual(
          actual.broker.FirstProcess,
          actual.broker.SecondProcess,
        );
        assert.equal(actual.generation, "FirstBoot");
        assert.equal(actual.accepted, "FirstProcess");
      },
    );
  },
);
