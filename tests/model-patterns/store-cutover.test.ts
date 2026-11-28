import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Store = "Source" | "Target";
type Command =
  | { kind: "seal" | "copy" | "cutover" }
  | {
      kind: "write";
      store: Store;
      generation: "Old" | "New";
      contents: "Initial" | "Changed";
    };
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/lease/store-cutover.ideal";

test("switching stores retains the fence and leaves the old destination closed", async () => {
  const base = await reachableScenario<Command>(checker, model, "TargetOpen");
  assert.deepEqual(base.commands, [
    { kind: "seal" },
    { kind: "copy" },
    { kind: "cutover" },
  ]);
  const commands: Command[] = [
    ...base.commands,
    { kind: "write", store: "Source", generation: "New", contents: "Changed" },
    { kind: "write", store: "Target", generation: "Old", contents: "Changed" },
    { kind: "write", store: "Target", generation: "New", contents: "Changed" },
  ];
  for (const fault of ["none", "body-only", "old-route-open"] as const) {
    const stores = {
      Source: new DatabaseSync(":memory:"),
      Target: new DatabaseSync(":memory:"),
    };
    for (const db of Object.values(stores)) {
      db.exec(
        "CREATE TABLE data (open INTEGER NOT NULL, floor INTEGER NOT NULL, body TEXT)",
      );
    }
    stores.Source.exec("INSERT INTO data VALUES (1, 2, 'Initial')");
    stores.Target.exec("INSERT INTO data VALUES (0, 1, NULL)");
    try {
      const result = await recordReplay<Command>(
        checker,
        { ...base, commands },
        {
          async apply(command) {
            switch (command.kind) {
              case "seal":
                stores.Source.exec("UPDATE data SET open = 0");
                break;
              case "copy": {
                const snapshot = stores.Source.prepare(
                  "SELECT body, floor FROM data WHERE open = 0",
                ).get();
                assert.ok(
                  snapshot &&
                    typeof snapshot.body === "string" &&
                    typeof snapshot.floor === "number",
                );
                if (fault === "body-only") {
                  stores.Target.prepare("UPDATE data SET body = ?").run(
                    snapshot.body,
                  );
                } else {
                  stores.Target.prepare(
                    "UPDATE data SET body = ?, floor = ?",
                  ).run(snapshot.body, snapshot.floor);
                }
                break;
              }
              case "cutover":
                stores.Target.exec("UPDATE data SET open = 1");
                break;
              case "write": {
                const ignoreClosure =
                  fault === "old-route-open" &&
                  command.store === "Source" &&
                  stores.Target.prepare("SELECT open FROM data").get()?.open ===
                    1;
                stores[command.store]
                  .prepare(
                    `UPDATE data SET body = ? WHERE floor <= ? AND (open = 1 OR ? = 1)`,
                  )
                  .run(
                    command.contents,
                    command.generation === "Old" ? 1 : 2,
                    Number(ignoreClosure),
                  );
                break;
              }
            }
          },
          async observe() {
            return {
              Source:
                stores.Source.prepare("SELECT body FROM data").get()?.body ??
                null,
              Target:
                stores.Target.prepare("SELECT body FROM data").get()?.body ??
                null,
            };
          },
          project: (state) => state.body,
        },
      );
      if (fault === "none") {
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          Source: "Initial",
          Target: "Changed",
        });
        assert.equal(
          stores.Target.prepare("SELECT floor FROM data").get()?.floor,
          2,
        );
      } else {
        assert.equal(result.verdict, "mismatch");
        assert.equal(
          result.difference.step,
          fault === "old-route-open" ? 4 : 5,
        );
        assert.deepEqual(
          result.difference.observed,
          fault === "old-route-open"
            ? { Source: "Changed", Target: "Initial" }
            : { Source: "Initial", Target: "Changed" },
        );
      }
    } finally {
      for (const db of Object.values(stores)) db.close();
    }
  }
});
