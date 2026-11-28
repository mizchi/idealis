import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Generation = "Old" | "New";
type Command =
  | { kind: "acquire" | "release" | "write"; generation: Generation }
  | { kind: "expire" };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/lease/fence.model";
const tokens = { Old: 1, New: 2 };
const payload = { Old: "old data", New: "new data" };

test("stale operations cannot alter a reissued lease or its data", async () => {
  const base = await reachableScenario<Command>(
    checker,
    model,
    "ReissuedBeforeFirstWrite",
  );
  assert.deepEqual(base.commands, [
    { kind: "acquire", generation: "Old" },
    { kind: "expire" },
    { kind: "acquire", generation: "New" },
  ]);
  for (const fault of ["none", "owner-only-release", "lazy-fence"] as const) {
    const db = new DatabaseSync(":memory:");
    db.exec(`CREATE TABLE lease (token INTEGER NOT NULL);
      CREATE TABLE sink (floor INTEGER NOT NULL, body TEXT);
      INSERT INTO sink VALUES (0, NULL)`);
    const commands: Command[] = [
      ...base.commands,
      { kind: "release", generation: "Old" },
      { kind: "write", generation: "Old" },
      { kind: "write", generation: "New" },
    ];
    try {
      const result = await recordReplay<Command>(
        checker,
        { ...base, commands },
        {
          async apply(command) {
            if (command.kind === "expire") {
              db.exec("DELETE FROM lease");
              return;
            }
            const token = tokens[command.generation];
            switch (command.kind) {
              case "acquire":
                db.exec("BEGIN");
                try {
                  db.prepare("INSERT INTO lease VALUES (?)").run(token);
                  if (fault !== "lazy-fence") {
                    db.prepare("UPDATE sink SET floor = MAX(floor, ?)").run(
                      token,
                    );
                  }
                  db.exec("COMMIT");
                } catch (error) {
                  db.exec("ROLLBACK");
                  throw error;
                }
                break;
              case "release":
                if (fault === "owner-only-release")
                  db.exec("DELETE FROM lease");
                else db.prepare("DELETE FROM lease WHERE token = ?").run(token);
                break;
              case "write":
                db.prepare(`UPDATE sink SET floor = MAX(floor, ?), body = ?
                WHERE floor <= ? AND EXISTS (SELECT 1 FROM lease)`).run(
                  token,
                  payload[command.generation],
                  token,
                );
                break;
            }
          },
          async observe() {
            return {
              lease: db.prepare("SELECT token FROM lease").get()?.token ?? null,
              body: db.prepare("SELECT body FROM sink").get()?.body ?? null,
            };
          },
          project: ({ active, written }) => ({
            lease: active === null ? null : tokens[active as Generation],
            body: written === null ? null : payload[written as Generation],
          }),
        },
      );
      if (fault === "none") {
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          lease: 2,
          body: "new data",
        });
      } else {
        assert.equal(result.verdict, "mismatch");
        assert.equal(
          result.difference.step,
          fault === "owner-only-release" ? 4 : 5,
        );
        assert.deepEqual(
          result.difference.observed,
          fault === "owner-only-release"
            ? { lease: null, body: null }
            : { lease: 2, body: "old data" },
        );
      }
    } finally {
      db.close();
    }
  }
});
