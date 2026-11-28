import assert from "node:assert/strict";
import test from "node:test";
import {
  recordReplay,
  replay,
  replayObservations,
  type Scenario,
} from "../../examples/model-patterns/replay.js";

const checkerPath = "crates/idealis-check/target/debug/idealis-check";
const scenario: Scenario<{ kind: "push"; destination: "Catalog" }> = {
  model: "examples/model-patterns/browser/model.project.json",
  config: { start: "Home" },
  initial: { current: "Home", nextRoute: null, atNext: false },
  commands: [{ kind: "push", destination: "Catalog" }],
};
const project = (state: Readonly<Record<string, unknown>>) => state.current;

test("recording preserves earlier observations if the adapter reuses an object", async () => {
  const observation = { current: "Home" };
  const result = await recordReplay(checkerPath, scenario, {
    async apply(command) {
      observation.current = command.destination;
    },
    async observe() {
      return observation;
    },
    project: (state) => ({ current: project(state) }),
  });
  assert.equal(result.verdict, "matched");
  assert.deepEqual(result.recording.observations, [
    { current: "Home" },
    { current: "Catalog" },
  ]);
});

test("recording propagates an environment error instead of returning a mismatch", async () => {
  const failure = new Error("connection closed");
  await assert.rejects(
    recordReplay(checkerPath, scenario, {
      async apply() {
        throw failure;
      },
      async observe() {
        return "Home";
      },
      project,
    }),
    (error) => error === failure,
  );
});

test("a saved observation reproduces the same mismatch without the environment", async () => {
  const observations: unknown[] = [];
  let mismatch: unknown;
  await assert.rejects(
    replay(
      checkerPath,
      scenario,
      {
        async apply() {},
        async observe() {
          return "Home";
        },
        project,
      },
      (observation) => observations.push(observation),
    ),
    (error) => {
      mismatch = error;
      return /"step":1.*"expected":"Catalog","observed":"Home"/.test(
        String(error),
      );
    },
  );
  assert.deepEqual(observations, ["Home", "Home"]);
  const saved = JSON.parse(JSON.stringify(observations)) as unknown[];
  await assert.rejects(
    replayObservations(checkerPath, scenario, saved, project),
    mismatch as Error,
  );
});

test("offline replay requires the initial observation and one per command", async () => {
  await replayObservations(checkerPath, scenario, ["Home", "Catalog"], project);
  await assert.rejects(
    replayObservations(checkerPath, scenario, ["Home"], project),
    /missing observation at step 1/,
  );
  await assert.rejects(
    replayObservations(
      checkerPath,
      scenario,
      ["Home", "Catalog", "Detail"],
      project,
    ),
    /unexpected observation at step 2/,
  );
});
