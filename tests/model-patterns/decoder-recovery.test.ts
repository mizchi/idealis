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
import type { Sequence, Wire } from "./helpers/decoder-recovery-child.js";

type Command =
  | { kind: "crash" | "restart" | "seal" | "retire" }
  | { kind: "receive"; sequence: Sequence }
  | { kind: "ack"; sequence: Sequence };
type Fault = "volatile-dedup" | "decode-first";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/decoder-recovery.model";

async function message(child: ChildProcess, command?: Wire) {
  const response = once(child, "message", {
    signal: AbortSignal.timeout(5_000),
  });
  if (command) child.send(command);
  return (await response)[0] as Sequence[];
}

async function stop(child: ChildProcess) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, "exit");
  assert.ok(child.kill("SIGKILL"));
  assert.deepEqual(await exited, [null, "SIGKILL"]);
}

async function openReceiver(t: TestContext, fault?: Fault) {
  const dir = await mkdtemp(join(tmpdir(), "idealis-decoder-recovery-"));
  const path = join(dir, "drain.sqlite");
  const db = new DatabaseSync(path);
  let child: ChildProcess | undefined;
  t.after(async () => {
    if (child) await stop(child);
    db.close();
    await rm(dir, { recursive: true, force: true });
  });
  db.exec(`
    CREATE TABLE progress (last INTEGER, retired INTEGER NOT NULL);
    INSERT INTO progress VALUES (NULL, 0);
    CREATE TABLE receipts (sequence TEXT PRIMARY KEY);
    CREATE TABLE effects (sequence TEXT NOT NULL);
    CREATE TABLE dropped (sequence TEXT PRIMARY KEY);
  `);

  // The broker keeps the transport envelope when the receiver process dies.
  const broker = new Map<Sequence, number[]>([
    ["First", [1, 1]],
    ["Second", [1, 1]],
  ]);
  const acked = new Set<Sequence>();
  const launch = () =>
    fork(
      fileURLToPath(
        new URL("./helpers/decoder-recovery-child.js", import.meta.url),
      ),
      [path, fault ?? "normal"],
      { stdio: ["ignore", "ignore", "inherit", "ipc"] },
    );
  let life: "Online" | "Offline" | "Restarted" = "Online";
  child = launch();
  let pendingAck = await message(child);
  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "crash":
          assert.ok(child);
          await stop(child);
          child = undefined;
          life = "Offline";
          pendingAck = [];
          break;
        case "restart":
          child = launch();
          pendingAck = await message(child);
          life = "Restarted";
          break;
        case "seal":
          assert.ok(child);
          pendingAck = await message(child, { kind: "seal", last: 2 });
          break;
        case "retire":
          assert.ok(child);
          pendingAck = await message(child, { kind: "retire" });
          break;
        case "receive": {
          assert.ok(child);
          const payload = broker.get(command.sequence);
          assert.ok(payload);
          pendingAck = await message(child, { ...command, payload });
          break;
        }
        case "ack":
          assert.ok(child);
          pendingAck = await message(child, command);
          assert.ok(broker.delete(command.sequence));
          acked.add(command.sequence);
          break;
      }
    },
    async observe() {
      const progress = db.prepare("SELECT * FROM progress").get();
      assert.ok(progress);
      return {
        life,
        decoder: progress.retired === 1 ? "CurrentOnly" : "Both",
        sealed: progress.last !== null,
        applied: db
          .prepare("SELECT sequence FROM receipts ORDER BY sequence")
          .all()
          .map((row) => row.sequence),
        dropped: db
          .prepare("SELECT sequence FROM dropped ORDER BY sequence")
          .all()
          .map((row) => row.sequence),
        effects: db.prepare("SELECT COUNT(*) AS count FROM effects").get()
          ?.count,
        pendingAck,
        acked: [...acked].sort(),
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "legacy retirement and deduplication survive a real receiver crash",
  { timeout: 30_000 },
  async (t) => {
    const replayed = await transitionScenario<Command>(
      checker,
      model,
      "receive",
      "sequence = First and before(life) = Restarted and before(decoder) = Both and First in before(applied) and unchanged(effects)",
    );
    await t.test(
      "the same outstanding frame has one durable effect",
      async (t) => {
        const normal = await recordReplay(
          checker,
          replayed,
          await openReceiver(t),
        );
        assert.equal(normal.verdict, "matched");
        const observed = normal.recording.observations.at(-1) as {
          effects: number;
          pendingAck: Sequence[];
        };
        assert.equal(observed.effects, 1);
        assert.deepEqual(observed.pendingAck, ["First"]);
        const faulty = await recordReplay(
          checker,
          replayed,
          await openReceiver(t, "volatile-dedup"),
        );
        assert.equal(faulty.verdict, "mismatch");
        assert.equal(faulty.difference.step, replayed.commands.length);
        assert.equal(
          (faulty.difference.observed as { effects: number }).effects,
          2,
        );
      },
    );

    await t.test(
      "restart restores the seal and waits for the missing sequence",
      async (t) => {
        const restarted = await transitionScenario<Command>(
          checker,
          model,
          "restart",
          "before(sealed) and before(applied) = First and after(decoder) = Both",
        );
        const run = await recordReplay(
          checker,
          {
            ...restarted,
            commands: [
              ...restarted.commands,
              { kind: "retire" },
              { kind: "receive", sequence: "Second" },
              { kind: "retire" },
            ],
          },
          await openReceiver(t),
        );
        assert.equal(run.verdict, "matched");
        const refused = run.recording.observations[
          restarted.commands.length + 1
        ] as {
          decoder: string;
          sealed: boolean;
        };
        assert.equal(refused.decoder, "Both");
        assert.equal(refused.sealed, true);
        assert.equal(
          (run.recording.observations.at(-1) as { decoder: string }).decoder,
          "CurrentOnly",
        );
      },
    );

    await t.test(
      "the transport can retry after retirement and restart",
      async (t) => {
        const restarted = await transitionScenario<Command>(
          checker,
          model,
          "restart",
          "after(decoder) = CurrentOnly and no before(acked)",
        );
        const receipt = { kind: "receive", sequence: "First" } as const;
        const ack = { kind: "ack", sequence: "First" } as const;
        const scenario = {
          ...restarted,
          commands: [...restarted.commands, receipt, ack],
        };
        const normal = await recordReplay(
          checker,
          scenario,
          await openReceiver(t),
        );
        assert.equal(normal.verdict, "matched");
        assert.deepEqual(normal.recording.observations.at(-1), {
          life: "Restarted",
          decoder: "CurrentOnly",
          sealed: true,
          applied: ["First", "Second"],
          effects: 2,
          dropped: [],
          pendingAck: [],
          acked: ["First"],
        });
        const faulty = await recordReplay(
          checker,
          scenario,
          await openReceiver(t, "decode-first"),
        );
        assert.equal(faulty.verdict, "mismatch");
        assert.equal(faulty.difference.step, restarted.commands.length + 1);
        assert.deepEqual(
          (faulty.difference.observed as { dropped: Sequence[] }).dropped,
          ["First"],
        );
      },
    );
  },
);
