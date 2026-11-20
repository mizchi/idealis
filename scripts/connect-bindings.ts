import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { extname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import type {
  ConnectContract,
  Field,
  Render,
} from "./connect-templates/contract.js";

// Subset of the pinned Quint typechecker IR consumed by the generator.
interface QuintType {
  kind: string;
  name?: string;
  fields?: {
    kind: string;
    fields: { fieldName: string; fieldType: QuintType }[];
    other: { kind: string };
  };
}
interface Declaration {
  kind: string;
  name: string;
  type?: QuintType;
  typeAnnotation?: QuintType;
}
interface CheckedModel {
  modules: { name: string; declarations: Declaration[] }[];
  errors: unknown[];
}

function row(type: QuintType | undefined, kind: string, path: string) {
  if (
    type?.kind !== kind ||
    type.fields?.kind !== "row" ||
    type.fields.other.kind !== "empty"
  )
    throw Error(
      `${path}: expected a closed ${kind}, got ${type?.kind ?? "missing"}`,
    );
  return type.fields.fields;
}
function record(type: QuintType | undefined, path: string): Field[] {
  const fields = row(type, "rec", path);
  if (!fields.length) throw Error(`${path}: empty records are unsupported`);
  return fields.map(({ fieldName: name, fieldType }) => {
    const kind = fieldType.kind;
    if (kind !== "int" && kind !== "bool" && kind !== "str")
      throw Error(`${path}.${name}: unsupported type ${kind}`);
    return { name, type: kind };
  });
}
function contract(
  model: CheckedModel,
  moduleName: string,
  bindings: ConnectContract["bindings"],
): ConnectContract {
  const module = model.modules.find((entry) => entry.name === moduleName);
  if (!module || model.errors.length)
    throw Error(`Expected a checked module ${moduleName}`);
  const declarations = module.declarations;
  function boundType(variable: string) {
    const annotation = declarations.find(
      (d) => d.kind === "var" && d.name === variable,
    )?.typeAnnotation;
    if (!annotation)
      throw Error(`Missing typed variable ${variable} in ${moduleName}`);
    if (annotation.kind !== "const") return annotation;
    return declarations.find(
      (d) => d.kind === "typedef" && d.name === annotation.name,
    )?.type;
  }
  const variants = row(boundType(bindings.command), "sum", "Command").map(
    ({ fieldName: name, fieldType }) => {
      const path = `Command.${name}`;
      const fields =
        fieldType.kind === "tup" && !row(fieldType, "tup", path).length
          ? []
          : record(fieldType, path);
      return { name, fields };
    },
  );
  if (!variants.length) throw Error("Command has no variants");
  return {
    version: 1,
    module: moduleName,
    bindings,
    variants,
    observation: record(boundType(bindings.observation), "Observation"),
  };
}

async function loadTemplate(template: string): Promise<Render> {
  const builtin =
    template === "ts" ? "typescript" : template === "rs" ? "rust" : undefined;
  // The same entrypoint also runs from dist after the workspace build.
  const url = builtin
    ? new URL(
        `./connect-templates/${builtin}${extname(import.meta.filename)}`,
        import.meta.url,
      )
    : pathToFileURL(resolve(template));
  const module = await import(url.href);
  if (typeof module.render !== "function")
    throw Error(`${template}: expected an exported render(contract) function`);
  return module.render;
}

async function main() {
  const { positionals, values } = parseArgs({
    allowPositionals: true,
    options: {
      command: { type: "string", default: "command" },
      observation: { type: "string", default: "observation" },
    },
  });
  const [source, moduleName, template, ...extra] = positionals;
  if (!source || !moduleName || !template || extra.length)
    throw Error(
      "Usage: connect-bindings <model.qnt> <module> <ts|rs|template.mjs>",
    );
  const render = await loadTemplate(template);
  const dir = await mkdtemp(join(tmpdir(), "idealis-quint-"));
  try {
    const output = join(dir, "checked.json");
    execFileSync(
      "pnpm",
      [
        "dlx",
        "@informalsystems/quint@0.33.1",
        "typecheck",
        resolve(source),
        "--out",
        output,
      ],
      { stdio: ["ignore", "pipe", "inherit"] },
    );
    const schema = contract(
      JSON.parse(await readFile(output, "utf8")),
      moduleName,
      values,
    );
    const rendered = await render(schema);
    if (typeof rendered !== "string")
      throw Error(`${template}: render() must return a string`);
    process.stdout.write(rendered);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}
main().catch((error: Error) => {
  console.error(error.message);
  process.exitCode = 1;
});
