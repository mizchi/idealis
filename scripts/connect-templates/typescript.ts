import type { ConnectContract, Field } from "./contract.js";

const scalars = { int: "bigint", bool: "boolean", str: "string" };
const tsFields = (fields: readonly Field[]) =>
  fields
    .map((f) => `readonly ${JSON.stringify(f.name)}: ${scalars[f.type]}`)
    .join("; ");
export function render({ variants, observation }: ConnectContract) {
  const union = variants
    .map(
      (v) =>
        `  | { readonly tag: ${JSON.stringify(v.name)}${v.fields.length ? `; readonly value: { ${tsFields(v.fields)} }` : ""} }`,
    )
    .join("\n");
  const handlers = variants.map((v) => {
    const arg = v.fields.length
      ? `input: Extract<Command, { tag: ${JSON.stringify(v.name)} }>["value"]`
      : "";
    return `    readonly ${JSON.stringify(v.name)}: (${arg}) => Awaitable<void>;`;
  });
  const cases = variants.map(
    (v) =>
      `    case ${JSON.stringify(v.name)}: return driver.actions[${JSON.stringify(v.name)}](${v.fields.length ? "command.value" : ""});`,
  );
  return `// Generated from Quint.
export type Command =\n${union};
export interface Observation { ${tsFields(observation)} }
type Awaitable<T> = T | Promise<T>;
export interface Driver {
  readonly actions: {
${handlers.join("\n")}
  };
  readonly observe: () => Awaitable<Observation>;
}
export function apply(driver: Driver, command: Command): Awaitable<void> {
  switch (command.tag) {
${cases.join("\n")}
  }
}
`;
}
