import assert from "node:assert/strict";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";

type Name = "Root" | "Middle" | "Target";
type Command = { kind: "replaceGraph" | "start" | "visit" | "finish" };

interface Node {
  readonly name: Name;
  next?: Node;
}

const checker = "crates/rcheck/target/debug/rcheck";
const model =
  "examples/model-patterns/worker-transfer/graph-version-reuse.model";

function graphWalk(compareLabelOnly: boolean): ObservationAdapter<Command> {
  const root: Node = { name: "Root", next: { name: "Middle" } };
  let label = "A";
  let generation = 0;
  let capturedLabel: string | null = null;
  let capturedGeneration: number | null = null;
  let cursor: Node | undefined;
  let seen = new Set<Node>();
  let published = false;

  function begin() {
    capturedLabel = label;
    capturedGeneration = generation;
    cursor = root;
    seen = new Set([root]);
  }

  return {
    async apply({ kind }) {
      switch (kind) {
        case "replaceGraph":
          generation++;
          label = generation === 1 ? "B" : "A";
          if (generation === 1) delete root.next;
          else root.next = { name: "Target" };
          break;
        case "start":
          begin();
          break;
        case "visit": {
          const next = cursor?.next;
          cursor = next && !seen.has(next) ? next : undefined;
          if (next) seen.add(next);
          break;
        }
        case "finish":
          if (
            compareLabelOnly
              ? capturedLabel === label
              : capturedGeneration === generation
          ) {
            published = true;
          } else begin();
          break;
      }
    },
    async observe() {
      return {
        label,
        capturedLabel,
        seen: [...seen].map(({ name }) => name).sort(),
        published,
      };
    },
    project: ({ label, capturedLabel, seen, published }) => ({
      label,
      capturedLabel,
      seen: [...(seen as string[])].sort(),
      published,
    }),
  };
}

test("an A → B → A graph label does not make a stale traversal publishable", async () => {
  const reused = await reachableScenario<Command>(
    checker,
    model,
    "ReturnedToCapturedLabel",
  );
  assert.equal(reused.commands.length, 5);
  const scenario = {
    ...reused,
    commands: [
      ...reused.commands,
      { kind: "finish" },
      { kind: "visit" },
      { kind: "visit" },
      { kind: "finish" },
    ],
  } satisfies typeof reused;
  const current = graphWalk(false);
  const normal = await recordReplay(checker, scenario, current);
  assert.equal(normal.verdict, "matched");
  assert.deepEqual(normal.recording.observations[5], {
    label: "A",
    capturedLabel: "A",
    seen: ["Middle", "Root"],
    published: false,
  });
  assert.deepEqual(await current.observe(), {
    label: "A",
    capturedLabel: "A",
    seen: ["Root", "Target"],
    published: true,
  });

  const wrong = await recordReplay(checker, scenario, graphWalk(true));
  assert.equal(wrong.verdict, "mismatch");
  assert.deepEqual(wrong.difference, {
    step: 6,
    command: { kind: "finish" },
    expected: {
      label: "A",
      capturedLabel: "A",
      seen: ["Root"],
      published: false,
    },
    observed: {
      label: "A",
      capturedLabel: "A",
      seen: ["Middle", "Root"],
      published: true,
    },
  });
});
