import assert from "node:assert/strict";
import { once } from "node:events";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import { MessageChannel } from "node:worker_threads";
import {
  recordReplay,
  transitionScenario,
} from "../../examples/model-patterns/replay.js";

type Attempt = "First" | "Second";
type Format = "Legacy" | "Current";
type Decision = "Accepted" | "Declined";
type Command =
  | { kind: "migrate" | "negotiate" }
  | { kind: "send" | "deliver"; attempt: Attempt };
type Fault = "use-preference" | "use-storage";
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/crash-recovery/receipt-wire.ideal";
const attempts = ["First", "Second"] as const;

function openClient(t: TestContext, initial: Decision, fault?: Fault) {
  const db = new DatabaseSync(":memory:");
  t.after(() => db.close());
  db.exec(`
    CREATE TABLE legacy (accepted INTEGER NOT NULL);
    CREATE TABLE current (decision TEXT NOT NULL);
  `);
  db.prepare("INSERT INTO legacy VALUES (?)").run(
    Number(initial === "Accepted"),
  );
  const pending = new Map<
    Attempt,
    { channel: MessageChannel; bytes: Uint8Array }
  >();
  const received = new Set<Attempt>();
  const conflicts = new Set<Attempt>();
  let preferred: Format = "Legacy";
  let accepted: Decision | null = null;
  let decoded: Decision | null = null;

  function storage(): Format {
    return db.prepare("SELECT 1 FROM current").get() ? "Current" : "Legacy";
  }
  function read(): Decision {
    const row = db.prepare("SELECT decision FROM current").get();
    if (row) {
      assert.ok(row.decision === "Accepted" || row.decision === "Declined");
      return row.decision;
    }
    return db.prepare("SELECT accepted FROM legacy").get()?.accepted === 1
      ? "Accepted"
      : "Declined";
  }
  function decode(format: Format, code: number): Decision {
    return code === (format === "Legacy" ? 1 : 0) ? "Accepted" : "Declined";
  }

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "migrate":
          db.exec(`
            INSERT INTO current SELECT CASE accepted WHEN 1 THEN 'Accepted' ELSE 'Declined' END FROM legacy;
          `);
          break;
        case "negotiate":
          preferred = "Current";
          break;
        case "send": {
          const channel = new MessageChannel();
          t.after(() => {
            channel.port1.close();
            channel.port2.close();
          });
          const legacy = preferred === "Legacy";
          const bytes = Uint8Array.of(
            legacy ? 1 : 2,
            Number((read() === "Accepted") === legacy),
          );
          pending.set(command.attempt, { channel, bytes });
          channel.port1.postMessage(bytes);
          break;
        }
        case "deliver": {
          const entry = pending.get(command.attempt);
          assert.ok(entry);
          const [bytes] = (await once(entry.channel.port2, "message")) as [
            Uint8Array,
          ];
          const [tag, code] = bytes;
          assert.ok((tag === 1 || tag === 2) && (code === 0 || code === 1));
          const wireFormat: Format = tag === 1 ? "Legacy" : "Current";
          const format =
            fault === "use-preference"
              ? preferred
              : fault === "use-storage"
                ? storage()
                : wireFormat;
          decoded = decode(format, code);
          if (accepted === null) accepted = decoded;
          else if (accepted !== decoded) conflicts.add(command.attempt);
          received.add(command.attempt);
          break;
        }
      }
    },
    async observe() {
      return {
        storage: storage(),
        preferred,
        sent: Object.fromEntries(
          attempts.map((attempt) => {
            const bytes = pending.get(attempt)?.bytes;
            const frame = bytes
              ? {
                  format: bytes[0] === 1 ? "Legacy" : "Current",
                  code: bytes[1] === 1 ? "One" : "Zero",
                }
              : null;
            return [attempt, frame];
          }),
        ),
        received: [...received].sort(),
        conflicts: [...conflicts].sort(),
        accepted,
        decoded,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "a queued response is decoded with its own wire format",
  { timeout: 30_000 },
  async (t) => {
    for (const { name, initial, action, predicate, fault, previous } of [
      {
        name: "legacy delivered after the newer reply",
        initial: "Accepted",
        action: "deliver",
        predicate:
          "attempt = First and result = Accepted and before(First.sent.format) = Legacy and before(Second.sent.format) = Current and Second in before(received)",
        fault: "use-preference",
        previous: "Accepted",
      },
      {
        name: "legacy delivered after storage migration and before negotiation",
        initial: "Declined",
        action: "migrate",
        predicate:
          "result = Declined and before(First.sent.format) = Legacy and no before(received)",
        fault: "use-storage",
        previous: null,
      },
    ] as const) {
      const route = await transitionScenario<Command>(
        checker,
        model,
        action,
        predicate,
      );
      const first: Command = { kind: "deliver", attempt: "First" };
      const scenario =
        action === "deliver"
          ? route
          : { ...route, commands: [...route.commands, first] };
      assert.deepEqual(scenario.commands.at(-1), first);
      await t.test(name, async (t) => {
        const run = await recordReplay(
          checker,
          scenario,
          openClient(t, initial),
        );
        assert.equal(run.verdict, "matched");
        assert.deepEqual(run.recording.observations.at(-1), {
          ...(run.recording.observations.at(-2) as Record<string, unknown>),
          received: previous ? ["First", "Second"] : ["First"],
          accepted: initial,
          decoded: initial,
          conflicts: [],
        });
      });
      if (previous) {
        await t.test("legacy delivered before the newer reply", async (t) => {
          const run = await recordReplay(
            checker,
            {
              ...scenario,
              commands: [
                ...scenario.commands.filter(
                  (command) => command.kind !== "deliver",
                ),
                first,
                { kind: "deliver", attempt: "Second" },
              ],
            },
            openClient(t, initial),
          );
          assert.equal(run.verdict, "matched");
          const actual = run.recording.observations.at(-1) as {
            accepted: Decision;
            conflicts: Attempt[];
          };
          assert.equal(actual.accepted, initial);
          assert.deepEqual(actual.conflicts, []);
        });
      }
      await t.test(fault, async (t) => {
        const run = await recordReplay(
          checker,
          scenario,
          openClient(t, initial, fault),
        );
        assert.equal(run.verdict, "mismatch");
        assert.equal(run.difference.step, scenario.commands.length);
        const actual = run.difference.observed as {
          decoded: Decision;
          accepted: Decision;
          conflicts: Attempt[];
        };
        assert.equal(
          actual.decoded,
          initial === "Accepted" ? "Declined" : "Accepted",
        );
        assert.equal(actual.accepted, previous ?? actual.decoded);
        assert.deepEqual(actual.conflicts, previous ? ["First"] : []);
      });
    }
  },
);
