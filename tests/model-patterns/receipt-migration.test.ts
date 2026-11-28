import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import {
  recordReplay,
  transitionScenario,
} from "../../examples/model-patterns/replay.js";

type Request = "Primary" | "Tail";
type Command =
  | { kind: "snapshot" | "restore" | "cutover" }
  | { kind: "submit" | "catchUp"; request: Request };
type Phase = "Legacy" | "Snapshot" | "Following" | "Current";
type Receipt = { body: "A" | "B"; issuedAs: "V1" | "V2" };
type Fault = "early-cutover" | "reissue-on-copy";
const requests = ["Primary", "Tail"] as const;
const bodyCode = { Primary: 1, Tail: 2 };

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/crash-recovery/receipt-migration.ideal";

function openStore(t: TestContext, fault?: Fault) {
  const db = new DatabaseSync(":memory:");
  t.after(() => db.close());
  db.exec(`
    CREATE TABLE v1 (request TEXT PRIMARY KEY, code INTEGER NOT NULL);
    CREATE TABLE captured (request TEXT PRIMARY KEY, code INTEGER NOT NULL);
    CREATE TABLE v2 (request TEXT PRIMARY KEY, receipt TEXT NOT NULL);
    CREATE TABLE effects (request TEXT NOT NULL);
  `);
  let phase: Phase = "Legacy";
  let reply: Receipt | null = null;
  function legacy(table: "v1" | "captured", request: Request): Receipt | null {
    const row = db
      .prepare(`SELECT code FROM ${table} WHERE request = ?`)
      .get(request);
    if (!row) return null;
    assert.ok(row.code === 1 || row.code === 2);
    return { body: row.code === 1 ? "A" : "B", issuedAs: "V1" };
  }
  function current(request: Request): Receipt | null {
    const row = db
      .prepare("SELECT receipt FROM v2 WHERE request = ?")
      .get(request);
    return row ? (JSON.parse(String(row.receipt)) as Receipt) : null;
  }
  function copy(table: "v1" | "captured", request: Request) {
    const receipt = legacy(table, request);
    if (receipt) {
      db.prepare("INSERT INTO v2 VALUES (?, ?)").run(
        request,
        JSON.stringify(
          fault === "reissue-on-copy"
            ? { ...receipt, issuedAs: "V2" }
            : receipt,
        ),
      );
    }
  }
  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "submit": {
          const request = command.request;
          const previous =
            phase === "Current" ? current(request) : legacy("v1", request);
          if (previous) {
            reply = previous;
            break;
          }
          db.exec("BEGIN");
          try {
            const code = bodyCode[request];
            db.prepare("INSERT INTO effects VALUES (?)").run(request);
            if (phase === "Current") {
              const receipt: Receipt = {
                body: code === 1 ? "A" : "B",
                issuedAs: "V2",
              };
              db.prepare("INSERT INTO v2 VALUES (?, ?)").run(
                request,
                JSON.stringify(receipt),
              );
              reply = receipt;
            } else {
              db.prepare("INSERT INTO v1 VALUES (?, ?)").run(request, code);
              reply = legacy("v1", request);
            }
            db.exec("COMMIT");
          } catch (error) {
            db.exec("ROLLBACK");
            throw error;
          }
          break;
        }
        case "snapshot":
          db.exec("INSERT INTO captured SELECT * FROM v1");
          phase = "Snapshot";
          break;
        case "restore":
          for (const request of requests) copy("captured", request);
          phase = "Following";
          break;
        case "catchUp":
          copy("v1", command.request);
          break;
        case "cutover": {
          const differs = db
            .prepare("SELECT request, code FROM v1")
            .all()
            .some((row) => {
              const migrated = current(row.request as Request);
              return (
                !migrated ||
                migrated.body !== (row.code === 1 ? "A" : "B") ||
                migrated.issuedAs !== "V1"
              );
            });
          if (!differs || fault === "early-cutover") phase = "Current";
          break;
        }
      }
    },
    async observe() {
      return {
        phase,
        old: Object.fromEntries(
          requests.map((request) => [request, legacy("v1", request)]),
        ),
        captured: Object.fromEntries(
          requests.map((request) => [request, legacy("captured", request)]),
        ),
        migrated: Object.fromEntries(
          requests.map((request) => [request, current(request)]),
        ),
        effects: db.prepare("SELECT COUNT(*) AS count FROM effects").get()
          ?.count,
        reply,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "receipt migration retains historical replies and copies writes after the snapshot",
  { timeout: 30_000 },
  async (t) => {
    const scenario = await transitionScenario<Command>(
      checker,
      model,
      "cutover",
      "some before(Primary.migrated) and some before(Tail.old) and no before(Tail.migrated) and unchanged(phase)",
    );
    assert.deepEqual(scenario.commands.at(-1), { kind: "cutover" });
    assert.equal(scenario.commands.length, 5);
    await t.test(
      "failed cutover can catch up and preserve both replies",
      async (t) => {
        const run = await recordReplay(
          checker,
          {
            ...scenario,
            commands: [
              ...scenario.commands,
              { kind: "catchUp", request: "Tail" },
              { kind: "cutover" },
              { kind: "submit", request: "Primary" },
              { kind: "submit", request: "Tail" },
            ],
          },
          openStore(t),
        );
        assert.equal(run.verdict, "matched");
        const [primary, tail] = run.recording.observations.slice(-2) as {
          phase: Phase;
          effects: number;
          reply: Receipt;
        }[];
        assert.deepEqual(
          [primary?.reply, tail?.reply],
          [
            { body: "A", issuedAs: "V1" },
            { body: "B", issuedAs: "V1" },
          ],
        );
        assert.equal(tail?.phase, "Current");
        assert.equal(tail?.effects, 2);
      },
    );
    await t.test(
      "a new request after cutover uses the current format",
      async (t) => {
        const write = await transitionScenario<Command>(
          checker,
          model,
          "submit",
          `request = Tail and before(phase) = Current and no before(Tail.migrated)
         and after(reply) = Receipt { body: B, issuedAs: V2 }`,
        );
        const run = await recordReplay(checker, write, openStore(t));
        assert.equal(run.verdict, "matched");
        assert.deepEqual(run.recording.observations.at(-1), {
          phase: "Current",
          old: { Primary: null, Tail: null },
          captured: { Primary: null, Tail: null },
          migrated: { Primary: null, Tail: { body: "B", issuedAs: "V2" } },
          effects: 1,
          reply: { body: "B", issuedAs: "V2" },
        });
      },
    );
    for (const fault of ["early-cutover", "reissue-on-copy"] as const) {
      await t.test(fault, async (t) => {
        const adapter = openStore(t, fault);
        const run = await recordReplay(checker, scenario, adapter);
        assert.equal(run.verdict, "mismatch");
        assert.deepEqual(run.difference.command, {
          kind: fault === "early-cutover" ? "cutover" : "restore",
        });
        if (fault === "early-cutover") {
          await adapter.apply({ kind: "submit", request: "Tail" });
          const state = await adapter.observe();
          assert.equal(state.effects, 3);
          assert.deepEqual(state.reply, { body: "B", issuedAs: "V2" });
        } else {
          const state = await adapter.observe();
          assert.deepEqual(state.migrated.Primary, {
            body: "A",
            issuedAs: "V2",
          });
        }
      });
    }
  },
);
