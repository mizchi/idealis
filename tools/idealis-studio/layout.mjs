import ELK from "elkjs/lib/elk.bundled.js";
import { layoutScore } from "./routing.mjs";
import { connections, edgeCaption } from "./topology.js";

const elk = new ELK({ algorithms: ["layered"] });

function labelSize(edge, names) {
  // Reserve room for either locale and for a single state shown by the filter.
  const widths = [new Map(), names.ja, names.en].flatMap((locale) =>
    [
      edgeCaption(edge, locale),
      ...edge.states.map((state) => edgeCaption(edge, locale, state)),
    ].map((caption) =>
      [...caption].reduce(
        (total, char) => total + (char.codePointAt(0) > 255 ? 14 : 9),
        0,
      ),
    ),
  );
  const longest = Math.max(...widths);
  const width = Math.max(58, Math.min(248, longest + 22));
  const lines =
    longest + 22 <= width ? 1 : Math.ceil(longest / (width - 40)) + 1;
  return { width, height: Math.max(26, lines * 16 + 10) };
}

export async function layoutGraph(components, names, domains = []) {
  const ownership = new Map(
    domains.flatMap(({ id, models }) => models.map((model) => [model, id])),
  );
  const models = components.filter(
    ({ name, ports }) => ownership.has(name) || ports.length > 0,
  );
  if (models.length === 0) return null;
  const edges = connections(models);
  const ids = new Map(models.map(({ name }, i) => [name, "model" + i]));

  const nodes = new Map(
    models.map(({ name }) => [
      name,
      { id: ids.get(name), width: 190, height: 94 },
    ]),
  );
  const boundaries = domains.map(({ models }, index) => ({
    id: "domain" + index,
    layoutOptions: { "elk.padding": "[top=57,left=25,bottom=25,right=25]" },
    children: models.map((model) => nodes.get(model)),
  }));

  const input = {
    id: "models",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.edgeRouting": "ORTHOGONAL",
      "elk.padding": "[top=36,left=36,bottom=36,right=36]",
      "elk.spacing.nodeNode": "45",
      "elk.spacing.componentComponent": "54",
      "elk.spacing.edgeLabel": "10",
      "elk.layered.spacing.nodeNodeBetweenLayers": "38",
      "elk.layered.spacing.edgeEdgeBetweenLayers": "22",
      "elk.layered.spacing.edgeNodeBetweenLayers": "20",
      "elk.layered.feedbackEdges": "true",
      ...(boundaries.length
        ? {
            "elk.hierarchyHandling": "INCLUDE_CHILDREN",
            "elk.json.shapeCoords": "ROOT",
            "elk.json.edgeCoords": "ROOT",
          }
        : {}),
    },
    children: [
      ...boundaries,
      ...models
        .filter(({ name }) => !ownership.has(name))
        .map(({ name }) => nodes.get(name)),
    ],
    edges: edges.map((edge, i) => ({
      id: "edge" + i,
      sources: [ids.get(edge.from)],
      targets: [ids.get(edge.to)],
      labels: [
        {
          id: "label" + i,
          text: edge.states[0],
          ...labelSize(edge, names),
          layoutOptions: {
            "elk.edgeLabels.placement": "CENTER",
            "elk.edgeLabels.inline": "true",
          },
        },
      ],
    })),
  };

  // ELK mutates its input graph. Keep candidates independent so a later layout
  // cannot replace the selected candidate's node or edge coordinates.
  const run = (layoutOptions = {}) => {
    const graph = structuredClone(input);
    Object.assign(graph.layoutOptions, layoutOptions);
    return elk.layout(graph);
  };
  let result = await run();
  if (edges.length > 0) {
    let best = layoutScore(result);
    for (const direction of boundaries.length ? ["RIGHT"] : ["RIGHT", "DOWN"]) {
      for (const [routing, cycle, feedback] of [
        ["ORTHOGONAL", "GREEDY", "true"],
        ["ORTHOGONAL", "GREEDY", "false"],
        ["ORTHOGONAL", "MODEL_ORDER", "false"],
        ["POLYLINE", "GREEDY", "false"],
        ["POLYLINE", "MODEL_ORDER", "false"],
      ]) {
        if (direction === "RIGHT" && feedback === "true") continue;
        const candidate = await run({
          "elk.direction": direction,
          "elk.edgeRouting": routing,
          "elk.layered.cycleBreaking.strategy": cycle,
          "elk.layered.feedbackEdges": feedback,
        });
        const score = layoutScore(candidate);
        if (
          score.overlaps < best.overlaps ||
          (score.overlaps === best.overlaps && score.cost < best.cost)
        ) {
          result = candidate;
          best = score;
        }
      }
    }
  }

  const groups = new Map(result.children.map((box) => [box.id, box]));
  const boxes = new Map(
    result.children
      .flatMap((box) => box.children ?? [box])
      .map((box) => [box.id, box]),
  );
  const routes = new Map(result.edges.map((route) => [route.id, route]));
  return {
    width: result.width,
    height: result.height,
    domains: domains.map(({ id, name }, index) => {
      const { x, y, width, height } = groups.get("domain" + index);
      return { id, name, x, y, width, height };
    }),
    nodes: models.map(({ name }) => {
      const { x, y, width, height } = boxes.get(ids.get(name));
      return { name, x, y, width, height, domain: ownership.get(name) };
    }),
    edges: edges.map((edge, i) => {
      const route = routes.get("edge" + i);
      const { x, y, width, height } = route.labels[0];
      return {
        ...edge,
        crossDomain: ownership.get(edge.from) !== ownership.get(edge.to),
        label: { x, y, width, height },
        paths: route.sections
          .toSorted(
            (a, b) =>
              Number(a.outgoingShape === ids.get(edge.to)) -
              Number(b.outgoingShape === ids.get(edge.to)),
          )
          .map(({ startPoint, bendPoints, endPoint }) => [
            startPoint,
            ...(bendPoints ?? []),
            endPoint,
          ]),
      };
    }),
  };
}
