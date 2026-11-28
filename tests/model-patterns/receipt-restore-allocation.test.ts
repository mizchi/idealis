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
  Envelope,
  Ticket,
  WorkerCommand,
  WorkerState,
} from "./helpers/restore-allocation-child.js";

type Command =
  | { kind: "reserve"; slot: Ticket }
  | { kind: "commit" | "send" | "crash" | "restart" }
  | { kind: "deliver"; origin: Ticket };
type Fault = "publish-early" | "volatile-counter";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model =
  "examples/model-patterns/crash-recovery/receipt-restore-allocation.ideal";
const oldReply = { kind: "deliver", origin: "First" } as const;
const commit = { kind: "commit" } as const;
const send = { kind: "send" } as const;
const restart = { kind: "restart" } as const;
const reserveFirst = { kind: "reserve", slot: "First" } as const;
const reserveSecond = { kind: "reserve", slot: "Second" } as const;

async function message(child: ChildProcess, command?: WorkerCommand) {
  const response = once(child, "message", {
    signal: AbortSignal.timeout(5000),
  });
  if (command) child.send(command);
  return (await response)[0] as WorkerState;
}

async function stop(child: ChildProcess) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, "exit");
  assert.ok(child.kill("SIGKILL"));
  assert.deepEqual(await exited, [null, "SIGKILL"]);
}

async function openAllocator(t: TestContext, fault?: Fault) {
  const root = await mkdtemp(join(tmpdir(), "idealis-restore-allocation-"));
  const path = join(root, "allocation.sqlite");
  const db = new DatabaseSync(path);
  db.exec(
    "CREATE TABLE allocation (high INTEGER NOT NULL); INSERT INTO allocation VALUES (0)",
  );
  let child: ChildProcess | undefined;
  t.after(async () => {
    if (child) await stop(child);
    db.close();
    await rm(root, { recursive: true, force: true });
  });
  const launch = () =>
    fork(
      fileURLToPath(
        new URL("./helpers/restore-allocation-child.js", import.meta.url),
      ),
      [path, fault ?? "normal"],
      { stdio: ["ignore", "ignore", "inherit", "ipc"] },
    );
  // The broker stays outside the receiver process and retains emitted envelopes.
  const broker = new Map<Ticket, Envelope>();
  const consumed = new Set<Ticket>();
  child = launch();
  let latest: WorkerState | null = await message(child);

  return {
    async apply(command: Command) {
      if (command.kind === "restart") {
        child = launch();
        latest = await message(child);
        return;
      }
      assert.ok(child);
      if (command.kind === "crash") {
        await stop(child);
        child = undefined;
        latest = null;
        return;
      }
      let outbound: WorkerCommand;
      if (command.kind === "reserve") {
        outbound = { kind: "reserve", ticket: command.slot };
      } else if (command.kind === "deliver") {
        const envelope = broker.get(command.origin);
        assert.ok(envelope);
        outbound = { kind: "deliver", envelope };
        consumed.add(command.origin);
      } else {
        assert.ok(command.kind === "commit" || command.kind === "send");
        outbound = { kind: command.kind };
      }
      latest = await message(child, outbound);
      if (latest.emitted) broker.set(latest.emitted.ticket, latest.emitted);
    },
    async observe() {
      return {
        phase: latest?.phase ?? "Offline",
        ticket: latest?.ticket ?? null,
        number: latest?.number ?? null,
        high: db.prepare("SELECT high FROM allocation").get()?.high,
        issued: {
          First: broker.get("First")?.number ?? null,
          Second: broker.get("Second")?.number ?? null,
        },
        consumed: [...consumed].sort(),
        accepted: latest?.accepted ?? null,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "the allocator commits an attempt ID before the broker can retain it",
  { timeout: 30000 },
  async (t) => {
    const rolledBack = await transitionScenario<Command>(
      checker,
      model,
      "crash",
      "before(phase) = Preparing and before(ticket) = First and before(high) = 0",
    );
    const committed = await transitionScenario<Command>(
      checker,
      model,
      "crash",
      "before(phase) = Durable and before(ticket) = First and before(high) = 1 and no before(issued)",
    );
    assert.deepEqual(
      rolledBack.commands.map(({ kind }) => kind),
      ["reserve", "crash"],
    );
    assert.deepEqual(
      committed.commands.map(({ kind }) => kind),
      ["reserve", "commit", "crash"],
    );
    for (const [scenario, number] of [
      [rolledBack, 1],
      [committed, 2],
    ] as const) {
      await t.test(
        `crash before send allows ID ${number} after restart`,
        async (t) => {
          const run = await recordReplay(
            checker,
            {
              ...scenario,
              commands: [
                ...scenario.commands,
                restart,
                reserveFirst,
                commit,
                send,
              ],
            },
            await openAllocator(t),
          );
          assert.equal(run.verdict, "matched");
          assert.deepEqual(run.recording.observations.at(-1), {
            phase: "Waiting",
            ticket: "First",
            number,
            high: number,
            issued: { First: number, Second: null },
            consumed: [],
            accepted: null,
          });
        },
      );
    }

    const delayed = await transitionScenario<Command>(
      checker,
      model,
      "deliver",
      "origin = First and before(ticket) = Second and before(phase) = Waiting and before(First.issued) = 1 and before(Second.issued) = 2 and unchanged(accepted)",
    );
    assert.equal(delayed.commands.length, 9);
    await t.test(
      "a delayed old reply stays distinct after SIGKILL",
      async (t) => {
        const run = await recordReplay(
          checker,
          {
            ...delayed,
            commands: [
              ...delayed.commands,
              { kind: "deliver", origin: "Second" },
            ],
          },
          await openAllocator(t),
        );
        assert.equal(run.verdict, "matched");
        const previous = run.recording.observations.at(-2) as {
          phase: string;
          accepted: Ticket | null;
        };
        assert.deepEqual(
          [previous.phase, previous.accepted],
          ["Waiting", null],
        );
        assert.deepEqual(run.recording.observations.at(-1), {
          phase: "Finished",
          ticket: "Second",
          number: 2,
          high: 2,
          issued: { First: 1, Second: 2 },
          consumed: ["First", "Second"],
          accepted: "Second",
        });
      },
    );

    await t.test(
      "dispatch before commit exposes a number that the DB can roll back",
      async (t) => {
        const adapter = await openAllocator(t, "publish-early");
        const run = await recordReplay(checker, rolledBack, adapter);
        assert.equal(run.verdict, "mismatch");
        assert.equal(run.difference.step, 1);
        for (const command of [
          ...rolledBack.commands.slice(1),
          restart,
          reserveSecond,
          commit,
          send,
          oldReply,
        ])
          await adapter.apply(command);
        const observed = await adapter.observe();
        assert.deepEqual(observed.issued, { First: 1, Second: 1 });
        assert.equal(observed.accepted, "First");
      },
    );
    await t.test(
      "a volatile counter reuses a sent ID after process replacement",
      async (t) => {
        const adapter = await openAllocator(t, "volatile-counter");
        const run = await recordReplay(checker, delayed, adapter);
        assert.equal(run.verdict, "mismatch");
        assert.equal(run.difference.step, 2);
        assert.deepEqual(run.difference.command, commit);
        for (const command of delayed.commands.slice(run.difference.step))
          await adapter.apply(command);
        const observed = await adapter.observe();
        assert.equal(observed.high, 0);
        assert.deepEqual(observed.issued, { First: 1, Second: 1 });
        assert.equal(observed.accepted, "First");
      },
    );
  },
);
