import assert from "node:assert/strict";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";

type Name = "Root" | "Middle" | "Target";
type Version = "Original" | "Replaced";
type Command = { kind: "replaceGraph" | "start" | "visit" | "finish" };
type Fault = "skip-version-check" | "keep-old-visits";

interface Node {
  readonly name: Name;
  next?: Node;
}

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/worker-transfer/graph-version.model";

function project({
  startedAt,
  cursor,
  seen,
  published,
}: Readonly<Record<string, unknown>>) {
  return { startedAt, cursor, seen: [...(seen as string[])].sort(), published };
}

function graphWalk(fault?: Fault): ObservationAdapter<Command> {
  const target: Node = { name: "Target" };
  const middle: Node = { name: "Middle" };
  const root: Node = { name: "Root", next: middle };
  let version: Version = "Original";
  let startedAt: Version | null = null;
  let cursor: Node | undefined;
  const seen = new Set<Node>();
  let published = false;

  function begin() {
    startedAt = version;
    cursor = root;
    seen.add(root);
  }

  return {
    async apply({ kind }) {
      switch (kind) {
        case "replaceGraph":
          delete root.next;
          middle.next = target;
          version = "Replaced";
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
          if (version === startedAt || fault === "skip-version-check") {
            published = true;
          } else {
            if (fault !== "keep-old-visits") seen.clear();
            begin();
          }
          break;
      }
    },
    async observe() {
      return {
        startedAt,
        cursor: cursor?.name ?? null,
        seen: [...seen].map(({ name }) => name).sort(),
        published,
      };
    },
    project,
  };
}

test("publishing a graph walk retries when its node reads span two versions", async () => {
  const stable = await reachableScenario<Command>(
    checker,
    model,
    "OriginalPublished",
  );
  const stableWalk = graphWalk();
  assert.equal(
    (await recordReplay(checker, stable, stableWalk)).verdict,
    "matched",
  );
  assert.deepEqual(await stableWalk.observe(), {
    startedAt: "Original",
    cursor: null,
    seen: ["Middle", "Root"],
    published: true,
  });

  const mixed = await reachableScenario<Command>(
    checker,
    model,
    "MixedAttemptNeedsRetry",
  );
  assert.deepEqual(
    mixed.commands.map(({ kind }) => kind),
    ["start", "visit", "replaceGraph", "visit", "visit"],
  );
  const retry = {
    ...mixed,
    commands: [
      ...mixed.commands,
      { kind: "finish" },
      { kind: "visit" },
      { kind: "finish" },
    ],
  } satisfies typeof mixed;
  const normal = graphWalk();
  const result = await recordReplay(checker, retry, normal);
  assert.equal(result.verdict, "matched");
  assert.deepEqual(result.recording.observations[5], {
    startedAt: "Original",
    cursor: null,
    seen: ["Middle", "Root", "Target"],
    published: false,
  });
  assert.deepEqual(await normal.observe(), {
    startedAt: "Replaced",
    cursor: null,
    seen: ["Root"],
    published: true,
  });

  for (const fault of ["skip-version-check", "keep-old-visits"] as const) {
    const result = await recordReplay(checker, retry, graphWalk(fault));
    assert.equal(result.verdict, "mismatch");
    assert.deepEqual(result.difference, {
      step: 6,
      command: { kind: "finish" },
      expected: {
        startedAt: "Replaced",
        cursor: "Root",
        seen: ["Root"],
        published: false,
      },
      observed: {
        startedAt: fault === "skip-version-check" ? "Original" : "Replaced",
        cursor: fault === "skip-version-check" ? null : "Root",
        seen: ["Middle", "Root", "Target"],
        published: fault === "skip-version-check",
      },
    });
  }
});
