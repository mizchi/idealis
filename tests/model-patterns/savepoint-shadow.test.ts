import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Value = "Original" | "First" | "Second" | "Staged";
type Label = "Outer" | "Inner";
type Command =
  | { kind: "begin" | "abort" | "commit" }
  | { kind: "mark" | "rewind" | "release"; label?: Label }
  | { kind: "write"; value: Value };
type Fault = "pop-after-rewind" | "commit-on-release" | "rewind-oldest";

const checker = "crates/rcheck/target/debug/rcheck";
const model =
  "examples/model-patterns/sqlite-transactions/savepoint-shadow.model";

async function openSavepoints(t: TestContext, fault?: Fault) {
  const directory = await mkdtemp(join(tmpdir(), "idealis-savepoints-"));
  const path = join(directory, "savepoints.sqlite");
  const writer = new DatabaseSync(path);
  const reader = new DatabaseSync(path);
  t.after(async () => {
    reader.close();
    writer.close();
    await rm(directory, { recursive: true, force: true });
  });
  writer.exec(`
    PRAGMA journal_mode = WAL;
    CREATE TABLE document (value TEXT NOT NULL);
    INSERT INTO document VALUES ('Original');
  `);
  const local = writer.prepare("SELECT value FROM document");
  const committed = reader.prepare("SELECT value FROM document");
  const update = writer.prepare("UPDATE document SET value = ?");
  const markers = { Outer: "outer_marker", Inner: "inner_marker" };
  return {
    async apply(command: Command) {
      const name =
        "label" in command && command.label ? markers[command.label] : "reuse";
      switch (command.kind) {
        case "begin":
          writer.exec("BEGIN IMMEDIATE");
          break;
        case "write":
          update.run(command.value);
          break;
        case "mark":
          writer.exec(`SAVEPOINT ${name}`);
          break;
        case "rewind":
          if (fault === "rewind-oldest") writer.exec(`RELEASE ${name}`);
          writer.exec(`ROLLBACK TO ${name}`);
          if (fault === "pop-after-rewind") writer.exec(`RELEASE ${name}`);
          break;
        case "release":
          writer.exec(
            fault === "commit-on-release" ? "COMMIT" : `RELEASE ${name}`,
          );
          break;
        case "abort":
          writer.exec("ROLLBACK");
          break;
        case "commit":
          writer.exec("COMMIT");
          break;
      }
    },
    async observe() {
      return {
        active: writer.isTransaction,
        local: local.get()?.value,
        committed: committed.get()?.value,
      };
    },
    project: ({
      active,
      local,
      committed,
    }: Readonly<Record<string, unknown>>) => ({
      active,
      local,
      committed,
    }),
  };
}

test(
  "SQLite keeps a same-name mark on rewind and leaves publication to the outer transaction",
  { timeout: 15_000 },
  async (t) => {
    const only = await reachableScenario<Command>(
      checker,
      model,
      "OnlyMarkWithStagedValue",
      { checker: "rprob" },
    );
    const shadowed = await reachableScenario<Command>(
      checker,
      model,
      "ShadowedMarkWithNewValue",
      { checker: "rprob" },
    );
    const repeatedRewind = {
      ...shadowed,
      commands: [
        ...shadowed.commands,
        { kind: "rewind" },
        { kind: "write", value: "Second" },
        { kind: "rewind" },
      ],
    } satisfies typeof shadowed;
    const rewindOuterAfterRelease = {
      ...shadowed,
      commands: [...shadowed.commands, { kind: "release" }, { kind: "rewind" }],
    } satisfies typeof shadowed;
    const abortAfterRelease = {
      ...only,
      commands: [...only.commands, { kind: "release" }, { kind: "abort" }],
    } satisfies typeof only;
    const commitAfterRelease = {
      ...only,
      commands: [...only.commands, { kind: "release" }, { kind: "commit" }],
    } satisfies typeof only;

    for (const [name, scenario, expected] of [
      [
        "two rewinds still select the inner mark",
        repeatedRewind,
        { active: true, local: "First", committed: "Original" },
      ],
      [
        "releasing the inner mark makes the outer mark visible",
        rewindOuterAfterRelease,
        { active: true, local: "Original", committed: "Original" },
      ],
      [
        "outer rollback removes the staged write",
        abortAfterRelease,
        { active: false, local: "Original", committed: "Original" },
      ],
      [
        "outer commit publishes the staged write",
        commitAfterRelease,
        { active: false, local: "First", committed: "First" },
      ],
    ] as const) {
      await t.test(name, async (t) => {
        const result = await recordReplay(
          checker,
          scenario,
          await openSavepoints(t),
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), expected);
      });
    }

    await t.test(
      "discarding the mark after rewind makes the next rewind restore the outer value",
      async (t) => {
        const result = await recordReplay(
          checker,
          repeatedRewind,
          await openSavepoints(t, "pop-after-rewind"),
        );
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, repeatedRewind.commands.length);
        assert.deepEqual(result.difference.observed, {
          active: true,
          local: "Original",
          committed: "Original",
        });
      },
    );
    await t.test(
      "committing on release publishes before the outer rollback",
      async (t) => {
        const result = await recordReplay(
          checker,
          abortAfterRelease,
          await openSavepoints(t, "commit-on-release"),
        );
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, only.commands.length + 1);
        assert.deepEqual(result.difference.observed, {
          active: false,
          local: "First",
          committed: "First",
        });
      },
    );
  },
);

test("a named savepoint skips unrelated marks but still selects the newest match", async (t) => {
  const named =
    "examples/model-patterns/sqlite-transactions/savepoint-named.model";
  const prefix = await reachableScenario<Command>(
    checker,
    named,
    "ShadowedMarkUnderOtherName",
    { checker: "rprob" },
  );
  const rewind = { kind: "rewind", label: "Outer" } as const;
  const first = { ...prefix, commands: [...prefix.commands, rewind] };
  const expected = { active: true, local: "Staged", committed: "Original" };
  const original = { ...expected, local: "Original" };

  await t.test(
    "rewind keeps the selected mark and discards the unrelated later mark",
    async (t) => {
      const driver = await openSavepoints(t);
      const result = await recordReplay(checker, first, driver);
      assert.equal(result.verdict, "matched");
      assert.deepEqual(result.recording.observations.at(-1), expected);
      await assert.rejects(
        driver.apply({ kind: "rewind", label: "Inner" }),
        /no such savepoint/,
      );
      assert.deepEqual(await driver.observe(), expected);
    },
  );

  await t.test(
    "release discards through the selected mark and exposes the older match",
    async (t) => {
      const driver = await openSavepoints(t);
      const afterRelease = {
        ...prefix,
        commands: [
          ...prefix.commands,
          { kind: "release", label: "Outer" },
          rewind,
        ],
      } satisfies typeof prefix;
      const result = await recordReplay(checker, afterRelease, driver);
      assert.equal(result.verdict, "matched");
      assert.deepEqual(result.recording.observations.at(-1), original);
    },
  );

  await t.test(
    "skipping the newest match restores an older value",
    async (t) => {
      const result = await recordReplay(
        checker,
        first,
        await openSavepoints(t, "rewind-oldest"),
      );
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, first.commands.length);
      assert.deepEqual(result.difference.expected, expected);
      assert.deepEqual(result.difference.observed, original);
    },
  );
});
