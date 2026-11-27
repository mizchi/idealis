import assert from "node:assert/strict";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type NodeName = "Root" | "Near" | "Bend" | "Deep" | "External" | "Orphan";
interface Node {
  name: NodeName;
  refs: Node[];
}
type Command = { kind: "attachSource" | "start" | "expand" | "finish" };
const checker = "crates/rcheck/target/debug/rcheck";
const model =
  "examples/model-patterns/worker-transfer/graph-reachability.model";

function traversal(useShortScan: boolean) {
  const deep: Node = { name: "Deep", refs: [] };
  const next: Node = { name: "Near", refs: [] };
  const bend: Node = { name: "Bend", refs: [deep, next] };
  next.refs.push(bend);
  const root: Node = { name: "Root", refs: [next] };
  const source: Node = { name: "External", refs: [] };
  let result: "Safe" | "Leaked" | null = null;
  let frontier: Node[] = [];
  let seen = new Set<Node>();
  let short = new Set<Node>();
  const names = (nodes: Iterable<Node>) =>
    [...nodes].map((node) => node.name).sort();
  return {
    async apply({ kind }: Command) {
      switch (kind) {
        case "attachSource":
          deep.refs.push(source);
          break;
        case "start":
          frontier = [root];
          seen = new Set(frontier);
          short = new Set([
            root,
            ...root.refs,
            ...root.refs.flatMap((node) => node.refs),
          ]);
          break;
        case "expand": {
          const nextLayer: Node[] = [];
          for (const node of frontier) {
            for (const ref of node.refs) {
              if (seen.has(ref)) continue;
              seen.add(ref);
              nextLayer.push(ref);
            }
          }
          frontier = nextLayer;
          break;
        }
        case "finish":
          assert.deepEqual(frontier, []);
          const reachable = new Set<Node>();
          function visit(node: Node) {
            if (reachable.has(node)) return;
            reachable.add(node);
            for (const ref of node.refs) visit(ref);
          }
          visit(root);
          assert.deepEqual(seen, reachable);
          result = (useShortScan ? short : seen).has(source)
            ? "Leaked"
            : "Safe";
          break;
      }
    },
    async observe() {
      return {
        seen: names(seen),
        frontier: names(frontier),
        short: names(short),
        result,
      };
    },
  };
}

test("a graph walk finds a deep link that a two-hop scan misses", async (t) => {
  const safe = await reachableScenario<Command>(
    checker,
    model,
    "FinishedWithoutSource",
  );
  const leak = await reachableScenario<Command>(
    checker,
    model,
    "FinishedWithSource",
  );
  assert.equal(safe.commands.length, 6);
  assert.equal(leak.commands.length, 8);
  for (const [scenario, useShortScan] of [
    [safe, false],
    [leak, false],
    [leak, true],
  ] as const) {
    await t.test(
      `${scenario === leak ? "deep source" : "isolated source"} / ${useShortScan ? "two hops" : "full walk"}`,
      async () => {
        const adapter = traversal(useShortScan);
        const result = await recordReplay(checker, scenario, {
          ...adapter,
          project: ({ seen, frontier, short, result }) => ({
            seen: [...(seen as string[])].sort(),
            frontier: [...(frontier as string[])].sort(),
            short: [...(short as string[])].sort(),
            result,
          }),
        });
        if (useShortScan) {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, 8);
        } else assert.equal(result.verdict, "matched");
        const expected =
          scenario === leak
            ? ["Bend", "Deep", "External", "Near", "Root"]
            : ["Bend", "Deep", "Near", "Root"];
        assert.deepEqual(await adapter.observe(), {
          seen: expected,
          frontier: [],
          short: ["Bend", "Near", "Root"],
          result: scenario === leak && !useShortScan ? "Leaked" : "Safe",
        });
      },
    );
  }
});
