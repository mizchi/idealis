// A project-owned convention: flat callbacks plus a read function.
export function render({ version, variants }) {
  if (version !== 1) throw new Error("Unsupported connect contract version");
  const fields = variants.map(({ name }) => {
    const callback = JSON.stringify("on" + name);
    return `  ${callback}: Driver["actions"][${JSON.stringify(name)}];`;
  });
  const actions = variants.map(({ name }) =>
    `      ${JSON.stringify(name)}: callbacks[${JSON.stringify("on" + name)}],`
  );
  return `import type { Driver } from "./bindings";

export function bind(callbacks: {
${fields.join("\n")}
  read: Driver["observe"];
}): Driver {
  return {
    actions: {
${actions.join("\n")}
    },
    observe: callbacks.read,
  };
}
`;
}
