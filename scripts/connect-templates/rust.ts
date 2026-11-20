import type { ConnectContract, Field } from "./contract.js";

const scalars = {
  int: "::num_bigint::BigInt",
  bool: "bool",
  str: "::std::string::String",
};
function rustRecord(name: string, fields: readonly Field[]) {
  for (const field of fields) {
    if (
      !/^[A-Za-z][A-Za-z0-9_]*$/.test(field.name) ||
      ["self", "super", "crate", "Self"].includes(field.name)
    )
      throw Error(`${name}.${field.name}: cannot be used as a Rust field name`);
  }
  const members = fields.map((f) => `    pub r#${f.name}: ${scalars[f.type]},`);
  return `#[derive(Debug, PartialEq, Eq)]
pub struct ${name} {
${members.join("\n")}
}`;
}

export function render({ variants: commands, observation }: ConnectContract) {
  const methodsSeen = new Set<string>();
  const variants = commands.map((command) => {
    if (!/^[A-Z][A-Za-z0-9]*$/.test(command.name) || command.name === "Self")
      throw Error(`Cannot emit Rust variant ${command.name}`);
    const method =
      "handle_" +
      command.name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
    if (methodsSeen.has(method))
      throw Error(`Duplicate Rust handler name: ${method}`);
    methodsSeen.add(method);
    return { ...command, method };
  });
  const payloads = variants
    .filter((v) => v.fields.length)
    .map((v) => rustRecord(v.name + "Input", v.fields));
  const members = variants.map(
    (v) => `    ${v.name}${v.fields.length ? `(${v.name}Input)` : ""},`,
  );
  const methods = variants.map((v) => {
    const parameter = v.fields.length ? `, input: ${v.name}Input` : "";
    return `    fn ${v.method}(&mut self${parameter}) -> Result<(), Self::Error>;`;
  });
  const cases = variants.map((v) => {
    const arg = v.fields.length ? "input" : "";
    return `            Self::${v.name}${arg ? "(input)" : ""} => driver.${v.method}(${arg}),`;
  });
  return `// Generated from Quint.
${payloads.join("\n")}
pub enum Command {
${members.join("\n")}
}
${rustRecord("Observation", observation)}
pub trait Driver {
    type Error;
${methods.join("\n")}
    fn observe(&self) -> Result<Observation, Self::Error>;
}
impl Command {
    pub fn apply<D: Driver>(self, driver: &mut D) -> Result<(), D::Error> {
        match self {
${cases.join("\n")}
        }
    }
}
`;
}
