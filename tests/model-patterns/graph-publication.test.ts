import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";

type Command = { kind: "prepare" | "check" | "replaceGraph" | "publish" };
type Generation = "Old" | "New";
type Node = "Root" | "Middle" | "Target";
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/worker-transfer/graph-publication.ideal";

async function graphPublisher(
  t: TestContext,
  trustEarlierCheck = false,
): Promise<ObservationAdapter<Command>> {
  const dir = await mkdtemp(join(tmpdir(), "idealis-graph-publication-"));
  const path = join(dir, "graph.sqlite");
  const reader = new DatabaseSync(path);
  const writer = new DatabaseSync(path);
  t.after(async () => {
    reader.close();
    writer.close();
    await rm(dir, { recursive: true, force: true });
  });
  writer.exec(`CREATE TABLE graph (generation TEXT NOT NULL, next TEXT NOT NULL);
    INSERT INTO graph VALUES ('Old', 'Middle');
    CREATE TABLE publication (generation TEXT NOT NULL, nodes TEXT NOT NULL)`);
  const readGraph = () =>
    reader.prepare("SELECT generation, next FROM graph").get() as {
      generation: Generation;
      next: Node;
    };
  let phase = "Idle";
  let candidate: { generation: Generation; nodes: readonly Node[] } | undefined;
  return {
    async apply({ kind }) {
      switch (kind) {
        case "prepare": {
          const { generation, next } = readGraph();
          candidate = { generation, nodes: ["Root", next] };
          phase = "Prepared";
          break;
        }
        case "check":
          assert.ok(candidate);
          phase =
            candidate.generation === readGraph().generation
              ? "Checked"
              : "Retry";
          break;
        case "replaceGraph":
          writer.exec("UPDATE graph SET generation = 'New', next = 'Target'");
          break;
        case "publish": {
          assert.ok(candidate);
          const insert = trustEarlierCheck
            ? "INSERT INTO publication VALUES ($generation, $nodes)"
            : "INSERT INTO publication SELECT generation, $nodes FROM graph WHERE generation = $generation";
          const { changes } = reader.prepare(insert).run({
            $generation: candidate.generation,
            $nodes: JSON.stringify(candidate.nodes),
          });
          phase = Number(changes) === 1 ? "Done" : "Retry";
          break;
        }
      }
    },
    async observe() {
      const receipt = writer
        .prepare("SELECT generation, nodes FROM publication")
        .get();
      return {
        generation: readGraph().generation,
        phase,
        publishedAt: receipt?.generation ?? null,
        publishedNodes: receipt
          ? (JSON.parse(String(receipt.nodes)) as Node[]).sort()
          : [],
      };
    },
    project: ({ generation, phase, publishedAt, publishedNodes }) => ({
      generation,
      phase,
      publishedAt,
      publishedNodes: [...(publishedNodes as string[])].sort(),
    }),
  };
}

test("publication rechecks the generation in the same SQLite statement that writes the result", async (t) => {
  const stale = await reachableScenario<Command>(
    checker,
    model,
    "GraphChangedAfterCheck",
  );
  assert.deepEqual(
    stale.commands.map(({ kind }) => kind),
    ["prepare", "check", "replaceGraph"],
  );
  const scenario = {
    ...stale,
    commands: [
      ...stale.commands,
      { kind: "publish" },
      { kind: "prepare" },
      { kind: "check" },
      { kind: "publish" },
    ],
  } satisfies typeof stale;
  const correct = await graphPublisher(t);
  const normal = await recordReplay(checker, scenario, correct);
  assert.equal(normal.verdict, "matched");
  assert.deepEqual(normal.recording.observations[4], {
    generation: "New",
    phase: "Retry",
    publishedAt: null,
    publishedNodes: [],
  });
  assert.deepEqual(await correct.observe(), {
    generation: "New",
    phase: "Done",
    publishedAt: "New",
    publishedNodes: ["Root", "Target"],
  });

  const faulty = await recordReplay(
    checker,
    scenario,
    await graphPublisher(t, true),
  );
  assert.equal(faulty.verdict, "mismatch");
  assert.deepEqual(faulty.difference, {
    step: 4,
    command: { kind: "publish" },
    expected: {
      generation: "New",
      phase: "Retry",
      publishedAt: null,
      publishedNodes: [],
    },
    observed: {
      generation: "New",
      phase: "Done",
      publishedAt: "Old",
      publishedNodes: ["Middle", "Root"],
    },
  });

  const fresh = await reachableScenario<Command>(
    checker,
    model,
    "GraphChangedAfterPublication",
  );
  const publisher = await graphPublisher(t);
  assert.equal(
    (await recordReplay(checker, fresh, publisher)).verdict,
    "matched",
  );
  assert.deepEqual(await publisher.observe(), {
    generation: "New",
    phase: "Done",
    publishedAt: "Old",
    publishedNodes: ["Middle", "Root"],
  });
});
