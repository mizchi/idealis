import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Topology = "Chain" | "Cycle";
type Row = "OrderRow" | "PaymentRow" | "ShipmentRow" | "AuditRow";
type Command =
  | { kind: "plan" | "publish" }
  | { kind: "fetch" | "disconnect" | "reconnect" | "expire"; row: Row };
type Decision =
  | "Waiting"
  | "Fetching"
  | "Retry"
  | "Expired"
  | "Ready"
  | "Published";
type Fault = "direct-only" | "rescan-all" | "expiry-as-retry" | null;

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/partition-repair.model";
const plan: Command = { kind: "plan" };
const publish: Command = { kind: "publish" };
const fetchPayment: Command = { kind: "fetch", row: "PaymentRow" };
const fetchOrder: Command = { kind: "fetch", row: "OrderRow" };

function openReplica(t: TestContext, topology: Topology, fault: Fault = null) {
  const db = new DatabaseSync(":memory:");
  t.after(() => db.close());
  db.exec(`
    CREATE TABLE source (
      row TEXT PRIMARY KEY,
      connected INTEGER DEFAULT 1,
      retained INTEGER DEFAULT 1
    );
    INSERT INTO source(row) VALUES ('OrderRow'), ('PaymentRow'), ('ShipmentRow'), ('AuditRow');
    CREATE TABLE dependencies (row TEXT, required TEXT, PRIMARY KEY (row, required));
    INSERT INTO dependencies VALUES ('ShipmentRow', 'PaymentRow'), ('PaymentRow', 'OrderRow');
    CREATE TABLE snapshots (row TEXT PRIMARY KEY);
    INSERT INTO snapshots VALUES ('ShipmentRow');
    CREATE TABLE planned (row TEXT PRIMARY KEY);
    CREATE TABLE replica (row TEXT PRIMARY KEY);
  `);
  if (topology === "Cycle") {
    db.exec("INSERT INTO dependencies VALUES ('OrderRow', 'ShipmentRow')");
  }

  let decision: Decision = "Waiting";
  function refresh() {
    db.exec("DELETE FROM planned");
    decision = "Waiting";
  }
  const dependencies =
    fault === "direct-only"
      ? "SELECT required AS row FROM dependencies JOIN snapshots USING (row)"
      : `WITH RECURSIVE needed(row) AS (
         SELECT required FROM dependencies JOIN snapshots USING (row)
         UNION
         SELECT dependencies.required FROM dependencies JOIN needed USING (row)
       )
       SELECT row FROM needed`;
  const planRows =
    fault === "rescan-all" ? "SELECT row FROM source" : dependencies;

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "plan": {
          db.exec(`INSERT INTO planned
                   SELECT row FROM (${planRows}) EXCEPT SELECT row FROM snapshots`);
          const missing = db
            .prepare(`
            SELECT retained, connected FROM planned JOIN source USING (row)
          `)
            .all();
          if (missing.length === 0) decision = "Ready";
          else if (missing.some((row) => row.retained === 0))
            decision = fault === "expiry-as-retry" ? "Retry" : "Expired";
          else if (missing.some((row) => row.connected === 1))
            decision = "Fetching";
          else decision = "Retry";
          break;
        }
        case "fetch": {
          const result = db
            .prepare(`
            INSERT INTO snapshots
            SELECT row FROM planned JOIN source USING (row)
            WHERE row = ? AND connected = 1 AND retained = 1
          `)
            .run(command.row);
          assert.equal(result.changes, 1);
          refresh();
          break;
        }
        case "disconnect":
          db.prepare("UPDATE source SET connected = 0 WHERE row = ?").run(
            command.row,
          );
          refresh();
          break;
        case "reconnect":
          db.prepare("UPDATE source SET connected = 1 WHERE row = ?").run(
            command.row,
          );
          refresh();
          break;
        case "expire":
          db.prepare("UPDATE source SET retained = 0 WHERE row = ?").run(
            command.row,
          );
          refresh();
          break;
        case "publish":
          db.exec("INSERT INTO replica SELECT row FROM snapshots");
          decision = "Published";
          break;
      }
    },
    async observe() {
      function rows(sql: string) {
        return db
          .prepare(sql)
          .all()
          .map((entry) => entry.row);
      }
      return {
        captured: rows("SELECT row FROM snapshots ORDER BY row"),
        pending: rows("SELECT row FROM planned ORDER BY row"),
        unavailable: rows(
          "SELECT row FROM source WHERE connected = 0 ORDER BY row",
        ),
        expired: rows("SELECT row FROM source WHERE retained = 0 ORDER BY row"),
        replica: rows("SELECT row FROM replica ORDER BY row"),
        decision,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "snapshot repair fetches the transitive dependencies of the selected cut",
  { timeout: 30_000 },
  async (t) => {
    for (const [topology, target] of [
      ["Chain", "ChainNeedsBoth"],
      ["Cycle", "CycleNeedsBoth"],
    ] as const) {
      await t.test(topology, async (t) => {
        const scenario = await reachableScenario<Command>(
          checker,
          model,
          target,
        );
        const result = await recordReplay(
          checker,
          {
            ...scenario,
            commands: [
              ...scenario.commands,
              fetchOrder,
              plan,
              fetchPayment,
              plan,
              publish,
            ],
          },
          openReplica(t, topology),
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          captured: ["OrderRow", "PaymentRow", "ShipmentRow"],
          pending: [],
          unavailable: [],
          expired: [],
          replica: ["OrderRow", "PaymentRow", "ShipmentRow"],
          decision: "Published",
        });
      });
    }

    await t.test(
      "retry waits for a missing row and can finish after reconnection",
      async (t) => {
        const scenario = await reachableScenario<Command>(
          checker,
          model,
          "AwaitingOrder",
        );
        const result = await recordReplay(
          checker,
          {
            ...scenario,
            commands: [
              ...scenario.commands,
              { kind: "reconnect", row: "OrderRow" },
              plan,
              fetchOrder,
              plan,
              publish,
            ],
          },
          openReplica(t, "Chain"),
        );
        assert.equal(result.verdict, "matched");
        assert.equal(
          (
            result.recording.observations[scenario.commands.length] as {
              decision: Decision;
            }
          ).decision,
          "Retry",
        );
        assert.equal(
          (result.recording.observations.at(-1) as { decision: Decision })
            .decision,
          "Published",
        );
      },
    );

    await t.test(
      "expiry of a prerequisite terminates this repair attempt",
      async (t) => {
        const scenario = await reachableScenario<Command>(
          checker,
          model,
          "ExpiredBeforePlan",
        );
        const withPlan = {
          ...scenario,
          commands: [...scenario.commands, plan],
        };
        const result = await recordReplay(
          checker,
          withPlan,
          openReplica(t, "Chain"),
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          captured: ["ShipmentRow"],
          pending: ["OrderRow", "PaymentRow"],
          unavailable: [],
          expired: ["OrderRow"],
          replica: [],
          decision: "Expired",
        });
        const faulty = await recordReplay(
          checker,
          withPlan,
          openReplica(t, "Chain", "expiry-as-retry"),
        );
        assert.equal(faulty.verdict, "mismatch");
        assert.equal(faulty.difference.step, withPlan.commands.length);
      },
    );

    await t.test(
      "retention expiry leaves a captured prerequisite usable",
      async (t) => {
        const scenario = await reachableScenario<Command>(
          checker,
          model,
          "ChainNeedsBoth",
        );
        const result = await recordReplay(
          checker,
          {
            ...scenario,
            commands: [
              ...scenario.commands,
              fetchOrder,
              { kind: "expire", row: "OrderRow" },
              plan,
              fetchPayment,
              plan,
              publish,
            ],
          },
          openReplica(t, "Chain"),
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          captured: ["OrderRow", "PaymentRow", "ShipmentRow"],
          pending: [],
          unavailable: [],
          expired: ["OrderRow"],
          replica: ["OrderRow", "PaymentRow", "ShipmentRow"],
          decision: "Published",
        });
      },
    );

    for (const fault of ["direct-only", "rescan-all"] as const) {
      await t.test(fault, async (t) => {
        const scenario = await reachableScenario<Command>(
          checker,
          model,
          "ChainNeedsBoth",
        );
        const result = await recordReplay(
          checker,
          scenario,
          openReplica(t, "Chain", fault),
        );
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, scenario.commands.length);
        assert.deepEqual(
          (result.difference.observed as { pending: string[] }).pending,
          fault === "direct-only"
            ? ["PaymentRow"]
            : ["AuditRow", "OrderRow", "PaymentRow"],
        );
      });
    }
  },
);
