import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command =
  | { kind: "append" | "seal" | "cutover" | "stop" | "restart" }
  | { kind: "apply"; position: number };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/lease/feed-cutover.model";

test("change feed restart preserves the cursor and cutover waits for the final position", async (t) => {
  const base = await reachableScenario<Command>(
    checker,
    model,
    "SourceWritesWhileReplicaStopped",
  );
  assert.deepEqual(base.commands, [
    { kind: "append" },
    { kind: "apply", position: 1 },
    { kind: "stop" },
    { kind: "append" },
  ]);
  const commands: Command[] = [
    ...base.commands,
    { kind: "restart" },
    { kind: "apply", position: 1 },
    { kind: "seal" },
    { kind: "cutover" },
    { kind: "apply", position: 2 },
    { kind: "cutover" },
  ];
  for (const fault of ["none", "duplicate-apply", "early-cutover"] as const) {
    const dir = await mkdtemp(join(tmpdir(), "idealis-change-feed-"));
    t.after(() => rm(dir, { recursive: true, force: true }));
    const source = new DatabaseSync(join(dir, "source.sqlite"));
    const path = join(dir, "target.sqlite");
    let target: DatabaseSync | null = new DatabaseSync(path);
    source.exec(`CREATE TABLE log (position INTEGER PRIMARY KEY, delta INTEGER NOT NULL);
      CREATE TABLE routing (sealed INTEGER, target INTEGER); INSERT INTO routing VALUES (0, 0)`);
    target.exec(
      "CREATE TABLE state (total INTEGER, cursor INTEGER); INSERT INTO state VALUES (0, 0)",
    );
    try {
      const result = await recordReplay<Command>(
        checker,
        { ...base, commands },
        {
          async apply(command) {
            switch (command.kind) {
              case "append":
                source.exec(
                  `INSERT INTO log SELECT (SELECT COALESCE(MAX(position), 0) + 1 FROM log), 1 FROM routing WHERE sealed = 0`,
                );
                break;
              case "stop":
                assert.ok(target);
                target.close();
                target = null;
                break;
              case "restart":
                assert.equal(target, null);
                target = new DatabaseSync(path);
                break;
              case "seal":
                source.exec("UPDATE routing SET sealed = 1");
                break;
              case "cutover": {
                assert.ok(target);
                const end = source
                  .prepare(
                    "SELECT COALESCE(MAX(position), 0) AS position FROM log",
                  )
                  .get()?.position;
                const applied = target
                  .prepare("SELECT cursor FROM state")
                  .get()?.cursor;
                if (applied === end || fault === "early-cutover")
                  source.exec("UPDATE routing SET target = 1 WHERE sealed = 1");
                break;
              }
              case "apply": {
                assert.ok(target);
                const entry = source
                  .prepare("SELECT delta FROM log WHERE position = ?")
                  .get(command.position);
                if (entry) {
                  assert.ok(typeof entry.delta === "number");
                  const predicate =
                    fault === "duplicate-apply"
                      ? "cursor + 1 >= ?"
                      : "cursor + 1 = ?";
                  target
                    .prepare(
                      `UPDATE state SET total = total + ?, cursor = MAX(cursor, ?) WHERE ${predicate}`,
                    )
                    .run(entry.delta, command.position, command.position);
                }
                break;
              }
            }
          },
          async observe() {
            const reader = new DatabaseSync(path, { readOnly: true });
            try {
              const state = reader
                .prepare("SELECT total, cursor FROM state")
                .get();
              const route =
                source.prepare("SELECT target FROM routing").get()?.target ===
                1;
              const feed = source
                .prepare(
                  "SELECT COALESCE(MAX(position), 0) AS head, COALESCE(SUM(delta), 0) AS total FROM log",
                )
                .get();
              assert.ok(feed);
              assert.ok(state);
              return {
                ...state,
                head: feed.head,
                route,
                liveValue: route ? state.total : feed.total,
              };
            } finally {
              reader.close();
            }
          },
          project: ({ total, cursor, head, routed }) => ({
            total,
            cursor,
            head,
            route: routed,
            liveValue: routed ? total : head,
          }),
        },
      );
      if (fault === "none") {
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          total: 2,
          cursor: 2,
          head: 2,
          route: true,
          liveValue: 2,
        });
      } else {
        assert.equal(result.verdict, "mismatch");
        assert.equal(
          result.difference.step,
          fault === "duplicate-apply" ? 6 : 8,
        );
        assert.deepEqual(
          result.difference.observed,
          fault === "duplicate-apply"
            ? { total: 2, cursor: 1, head: 2, route: false, liveValue: 2 }
            : { total: 1, cursor: 1, head: 2, route: true, liveValue: 1 },
        );
      }
    } finally {
      target?.close();
      source.close();
    }
  }
});
