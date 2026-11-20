import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  createNative,
  maxWork,
  wasmPath,
} from "../../evals/experiments/alloy-subset/clients.js";
import {
  generated,
  handCases,
  source,
} from "../../evals/experiments/alloy-subset/corpus.js";
import { verify } from "../../evals/experiments/alloy-subset/verify.js";
import { createWasm } from "../../evals/experiments/alloy-subset/wasm.js";
import { analysis } from "../../evals/experiments/alloy-subset/wire.js";

test("ralloy native and pure Wasm agree on relations, scope, controls and budgets", async () => {
  await verify(
    [...handCases(), ...generated(0x34a1, 8)],
    await createWasm(await readFile(wasmPath)),
    () => {},
  );
});

test("an Alloy counterexample reproduces a known cycle validation defect", async () => {
  const native = createNative();
  try {
    const input = handCases()[0];
    assert.ok(input);
    const result = analysis(
      await native.request({
        kind: "analyze",
        source: source(input),
        command: 0,
        maxWork,
      }),
    );
    assert.equal(result.status, "counterexample");
    assert.ok(result.instance);
    const nodes = result.instance.signatures.Node;
    const edges = result.instance.fields.edge;
    assert.ok(nodes && edges);
    const broken = edges.every((edge) => edge[0] !== edge[1]);
    const reaches = (
      start: string,
      current: string,
      visited: ReadonlySet<string>,
    ): boolean =>
      edges
        .filter((edge) => edge[0] === current)
        .some(
          (edge) =>
            edge[1] === start ||
            (!visited.has(edge[1]) &&
              reaches(start, edge[1], new Set([...visited, edge[1]]))),
        );
    const fixed = !nodes.some((node) => reaches(node, node, new Set([node])));
    assert.equal(broken, true);
    assert.equal(fixed, false);
  } finally {
    await native.close();
  }
});
