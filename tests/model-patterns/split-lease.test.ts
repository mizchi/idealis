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

type Generation = "Old" | "New";
type Command =
  | {
      kind: "prepare" | "install" | "confirm" | "activate" | "stop" | "restart";
    }
  | { kind: "write"; generation: Generation };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/lease/split-fence.model";
const token = { Old: 1, New: 2 };

test("a recovering coordinator verifies the separate durable fence before activating a lease", async (t) => {
  const beforeInstall = await reachableScenario<Command>(
    checker,
    model,
    "StoppedBeforeInstall",
  );
  const afterInstall = await reachableScenario<Command>(
    checker,
    model,
    "StoppedAfterInstall",
  );
  assert.equal(afterInstall.commands.at(-1)?.kind, "stop");
  const afterConfirm = {
    ...afterInstall,
    commands: [
      ...afterInstall.commands.slice(0, -1),
      { kind: "confirm" },
      { kind: "stop" },
    ] satisfies Command[],
  };
  const cases = [
    [beforeInstall, false, 1],
    [afterInstall, false, 2],
    [afterConfirm, false, 2],
    [beforeInstall, true, 0],
  ] as const;
  for (const [base, trustPrepared, expectedAttempts] of cases) {
    const dir = await mkdtemp(join(tmpdir(), "idealis-split-lease-"));
    t.after(() => rm(dir, { recursive: true, force: true }));
    const path = join(dir, "coordinator.sqlite");
    let coordinator: DatabaseSync | null = new DatabaseSync(path);
    const sink = new DatabaseSync(join(dir, "sink.sqlite"));
    coordinator.exec(
      "CREATE TABLE lease (active INTEGER, prepared INTEGER); INSERT INTO lease VALUES (1, NULL)",
    );
    sink.exec(
      "CREATE TABLE data (floor INTEGER NOT NULL, written INTEGER); INSERT INTO data VALUES (1, NULL)",
    );
    let confirmed = false;
    let attempts = 0;
    function publish(generation: Generation) {
      sink
        .prepare("UPDATE data SET written = ? WHERE floor = ?")
        .run(token[generation], token[generation]);
    }
    const commands: Command[] = [
      ...base.commands,
      { kind: "restart" },
      { kind: "install" },
      { kind: "confirm" },
      { kind: "activate" },
      { kind: "write", generation: "Old" },
      { kind: "write", generation: "New" },
    ];
    try {
      const result = await recordReplay<Command>(
        checker,
        { ...base, commands },
        {
          async apply(command) {
            switch (command.kind) {
              case "prepare":
                assert.ok(coordinator);
                coordinator.exec(
                  "UPDATE lease SET active = NULL, prepared = 2",
                );
                break;
              case "install":
                attempts++;
                sink.exec("UPDATE data SET floor = MAX(floor, 2)");
                break;
              case "confirm":
                confirmed =
                  sink.prepare("SELECT floor FROM data").get()?.floor === 2;
                break;
              case "activate":
                assert.ok(coordinator && confirmed);
                coordinator.exec(
                  "UPDATE lease SET active = prepared, prepared = NULL",
                );
                confirmed = false;
                break;
              case "stop":
                assert.ok(coordinator);
                coordinator.close();
                coordinator = null;
                confirmed = false;
                break;
              case "restart":
                assert.equal(coordinator, null);
                coordinator = new DatabaseSync(path);
                if (trustPrepared)
                  coordinator.exec(
                    "UPDATE lease SET active = prepared, prepared = NULL",
                  );
                break;
              case "write":
                publish(command.generation);
                break;
            }
          },
          async observe() {
            const reader = new DatabaseSync(path, { readOnly: true });
            try {
              const lease = reader
                .prepare("SELECT active, prepared FROM lease")
                .get();
              const stored = sink
                .prepare("SELECT floor, written FROM data")
                .get();
              assert.ok(lease && stored);
              return {
                ...lease,
                ...stored,
                confirmed,
                online: coordinator !== null,
              };
            } finally {
              reader.close();
            }
          },
          project: ({
            active,
            prepared,
            floor,
            written,
            confirmed,
            online,
          }) => {
            const encode = (value: unknown) =>
              value === null ? null : token[value as Generation];
            return {
              active: encode(active),
              prepared: encode(prepared),
              floor: encode(floor),
              written: encode(written),
              confirmed,
              online,
            };
          },
        },
      );
      assert.equal(attempts, expectedAttempts);
      if (trustPrepared) {
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, base.commands.length + 1);
        assert.deepEqual(result.difference.observed, {
          active: 2,
          prepared: null,
          floor: 1,
          written: null,
          confirmed: false,
          online: true,
        });
        publish("Old");
        assert.equal(
          sink.prepare("SELECT written FROM data").get()?.written,
          1,
        );
      } else {
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          active: 2,
          prepared: null,
          floor: 2,
          written: 2,
          confirmed: false,
          online: true,
        });
      }
    } finally {
      coordinator?.close();
      sink.close();
    }
  }
});
