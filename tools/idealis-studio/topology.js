// Project ports describe shared state. An arrow follows writes to reads.
export function connections(components) {
  const byState = new Map();
  for (const component of components) {
    for (const { state, access } of component.ports) {
      if (typeof state !== "string" || !["write", "read"].includes(access))
        continue;
      if (!byState.has(state))
        byState.set(state, { write: new Set(), read: new Set() });
      byState.get(state)[access].add(component.name);
    }
  }

  const graph = new Map(components.map(({ name }) => [name, new Map()]));
  for (const [state, { write, read }] of byState) {
    for (const from of write) {
      for (const to of read) {
        if (from === to) continue;
        const targets = graph.get(from);
        if (!targets.has(to)) targets.set(to, []);
        targets.get(to).push(state);
      }
    }
  }
  return [...graph].flatMap(([from, targets]) =>
    [...targets].map(([to, states]) => ({ from, to, states: states.sort() })),
  );
}

export function stateNames(terms, model, language) {
  const names = new Map();
  for (const term of terms) {
    const name = term.name?.[language];
    if (!name) continue;
    for (const link of Array.isArray(term.model) ? term.model : []) {
      if (link.path === model && !names.has(link.name))
        names.set(link.name, name);
    }
  }
  return names;
}

export function edgeCaption(edge, names, state) {
  if (state) return names.get(state) ?? state;
  const first = edge.states[0];
  return (
    (names.get(first) ?? first) +
    (edge.states.length > 1 ? " +" + (edge.states.length - 1) : "")
  );
}

function svgElement(tag, attributes) {
  const element = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [name, value] of Object.entries(attributes))
    element.setAttribute(name, value);
  return element;
}

export function wireLayer(layout, visible, filtered) {
  const svg = svgElement("svg", {
    viewBox: `0 0 ${layout.width} ${layout.height}`,
    "aria-hidden": "true",
  });
  const variants = [
    "normal",
    "active",
    "dimmed",
    "boundary",
    "boundary-active",
  ];
  const defs = svgElement("defs", {});
  for (const variant of variants) {
    const marker = svgElement("marker", {
      id: "graph-arrow-" + variant,
      viewBox: "0 0 10 10",
      refX: "10",
      refY: "5",
      markerWidth: "5",
      markerHeight: "5",
      orient: "auto",
    });
    marker.append(
      svgElement("path", {
        d: "M0,0 L10,5 L0,10 Z",
        class: "wire-arrow wire-" + variant,
      }),
    );
    defs.append(marker);
  }
  svg.append(defs);
  for (const edge of [...layout.edges].sort(
    (a, b) => Number(visible.has(a)) - Number(visible.has(b)),
  )) {
    const variant = visible.has(edge)
      ? edge.crossDomain
        ? filtered
          ? "boundary-active"
          : "boundary"
        : filtered
          ? "active"
          : "normal"
      : "dimmed";
    for (const [index, points] of edge.paths.entries()) {
      const path = svgElement("polyline", {
        points: points.map(({ x, y }) => `${x},${y}`).join(" "),
        class: "graph-wire wire-" + variant,
      });
      if (index === edge.paths.length - 1)
        path.setAttribute("marker-end", `url(#graph-arrow-${variant})`);
      svg.append(path);
    }
  }
  return svg;
}
