// Compare layouts in model coordinates, independently of zoom and selection.
const epsilon = 0.001;
const difference = (from, to) => ({ x: to.x - from.x, y: to.y - from.y });
const cross = (a, b) => a.x * b.y - a.y * b.x;
const dot = (a, b) => a.x * b.x + a.y * b.y;
const length = ({ x, y }) => Math.hypot(x, y);

function routeMetrics(edges) {
  const segments = [];
  let bends = 0;
  let wireLength = 0;
  let backtrack = 0;
  for (const edge of edges) {
    for (const { startPoint, bendPoints = [], endPoint } of edge.sections) {
      const points = [startPoint, ...bendPoints, endPoint];
      const axis = difference(startPoint, endPoint);
      const span = length(axis);
      let previous;
      for (let i = 1; i < points.length; i++) {
        const from = points[i - 1];
        const to = points[i];
        const step = difference(from, to);
        const distance = length(step);
        if (distance < epsilon) continue;
        if (
          previous &&
          (Math.abs(cross(previous, step)) > epsilon || dot(previous, step) < 0)
        ) {
          bends++;
        }
        wireLength += distance;
        if (span > epsilon) backtrack += Math.max(0, -dot(step, axis) / span);
        previous = step;
        segments.push({
          edge,
          from,
          to,
          step,
          left: Math.min(from.x, to.x),
          right: Math.max(from.x, to.x),
        });
      }
    }
  }

  let crossings = 0;
  segments.sort((a, b) => a.left - b.left);
  for (const [index, a] of segments.entries()) {
    for (
      let i = index + 1;
      i < segments.length && segments[i].left <= a.right;
      i++
    ) {
      const b = segments[i];
      if (a.edge === b.edge) continue;
      const c = cross(a.step, difference(a.from, b.from));
      const d = cross(a.step, difference(a.from, b.to));
      const e = cross(b.step, difference(b.from, a.from));
      const f = cross(b.step, difference(b.from, a.to));
      if (c * d < -epsilon && e * f < -epsilon) crossings++;
    }
  }
  return { bends, wireLength, backtrack, crossings };
}

function countOverlaps(boxes) {
  const ordered = boxes.toSorted((a, b) => a.x - b.x);
  let overlaps = 0;
  for (const [index, a] of ordered.entries()) {
    for (
      let i = index + 1;
      i < ordered.length && ordered[i].x < a.x + a.width - epsilon;
      i++
    ) {
      const b = ordered[i];
      if (b.y < a.y + a.height - epsilon && a.y < b.y + b.height - epsilon)
        overlaps++;
    }
  }
  return overlaps;
}

export function layoutScore(graph) {
  const overlaps = countOverlaps([
    ...graph.children.flatMap((node) => node.children ?? [node]),
    ...graph.edges.flatMap(({ labels }) => labels),
  ]);
  const { bends, wireLength, backtrack, crossings } = routeMetrics(graph.edges);
  // Width and height also matter: a very long diagram costs zoom even if its edges are straight.
  const canvasExtent = Math.max(graph.width / 1.8, graph.height);
  const cost =
    bends * 32 +
    backtrack * 0.5 +
    crossings * 75 +
    wireLength * 0.035 +
    canvasExtent * 0.75;
  return { overlaps, cost };
}
