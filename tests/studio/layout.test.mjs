import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { layoutGraph } from "../../tools/idealis-studio/layout.mjs";
import { stateNames } from "../../tools/idealis-studio/topology.js";

function intersects(a, b) {
  return (
    a.x < b.x + b.width &&
    b.x < a.x + a.width &&
    a.y < b.y + b.height &&
    b.y < a.y + a.height
  );
}

test("a larger cyclic model reserves space for every node and localized label", async () => {
  const components = Array.from({ length: 24 }, (_, i) => ({
    name: "part" + i,
    ports: [],
  }));
  const names = { ja: new Map(), en: new Map() };
  function connect(from, to) {
    const state = `state_${from}_${to}`;
    components[from].ports.push({ state, access: "write" });
    components[to].ports.push({ state, access: "read" });
    names.ja.set(
      state,
      `共有状態 ${from} から ${to} への反映通知と受け付け済み項目`,
    );
    names.en.set(
      state,
      `Accepted result and acknowledgement sent from model ${from} to model ${to}`,
    );
  }
  for (let row = 0; row < 5; row++) {
    for (let col = 0; col < 4; col++) {
      const index = row * 4 + col;
      connect(index, index + 4);
      if (col < 3) connect(index, index + 5);
    }
  }
  connect(5, 0);
  connect(18, 10);
  const layout = await layoutGraph(components, names);
  assert.equal(layout.nodes.length, 24);
  assert.equal(layout.edges.length, 37);
  assert.ok(
    layout.width / layout.height < 3,
    "use the more compact direction for this larger graph",
  );
  const boxes = [...layout.nodes, ...layout.edges.map(({ label }) => label)];
  for (const [i, box] of boxes.entries()) {
    assert.ok(box.x >= 0 && box.y >= 0);
    assert.ok(
      box.x + box.width <= layout.width && box.y + box.height <= layout.height,
    );
    assert.ok(
      boxes.slice(0, i).every((other) => !intersects(box, other)),
      JSON.stringify(box),
    );
  }
  for (const edge of layout.edges) {
    assert.ok(
      edge.label.height >= 36,
      "reserve two or more lines for long localized labels",
    );
    for (const points of edge.paths) {
      assert.ok(points.length >= 2);
      assert.ok(
        points.every(
          (point) => Number.isFinite(point.x) && Number.isFinite(point.y),
        ),
      );
    }
  }
});

test("microblog routes avoid the former feedback detours", async () => {
  const root = "examples/model-patterns/worker-microblog";
  const model = root + "/model.project.json";
  const { fragments } = JSON.parse(await readFile(model, "utf8"));
  const {
    spec: { terms },
  } = JSON.parse(await readFile(root + "/spec.json", "utf8"));
  const components = fragments
    .filter((fragment) => typeof fragment === "object")
    .map((fragment) => ({
      name: fragment.namespace,
      ports: Object.values(fragment.ports),
    }));
  const layout = await layoutGraph(components, {
    ja: stateNames(terms, model, "ja"),
    en: stateNames(terms, model, "en"),
  });
  assert.equal(layout.edges.length, 9);
  let turns = 0;
  let length = 0;
  for (const edge of layout.edges) {
    for (const path of edge.paths) {
      const first = path[0];
      const last = path.at(-1);
      const axis = { x: last.x - first.x, y: last.y - first.y };
      for (let i = 1; i < path.length; i++) {
        const from = path[i - 1];
        const to = path[i];
        const dx = to.x - from.x;
        const dy = to.y - from.y;
        assert.ok(
          dx * axis.x + dy * axis.y >= -0.01,
          `${edge.from} → ${edge.to} backtracks`,
        );
        length += Math.hypot(dx, dy);
        if (
          i > 1 &&
          Math.abs(
            (from.x - path[i - 2].x) * dy - (from.y - path[i - 2].y) * dx,
          ) > 0.001
        )
          turns++;
      }
    }
  }
  assert.ok(turns <= 16, `${turns} bends`);
  assert.ok(length < 4200, `${length} units of wire`);
});
