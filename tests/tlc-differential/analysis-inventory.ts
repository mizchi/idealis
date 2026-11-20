import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";

interface AstNode {
  readonly text?: string;
  readonly name?: AstNode;
  readonly type?: AstNode;
  readonly types?: readonly AstNode[];
  readonly members?: readonly AstNode[];
  readonly initializer?: AstNode;
  readonly questionToken?: AstNode;
  getText(source?: AstNode): string;
  forEachChild(visitor: (node: AstNode) => void): void;
}

interface TypeScriptApi {
  readonly ScriptTarget: { readonly Latest: number };
  createSourceFile(
    name: string,
    source: string,
    target: number,
    parents: boolean,
  ): AstNode;
  isInterfaceDeclaration(node: AstNode): boolean;
  isTypeAliasDeclaration(node: AstNode): boolean;
  isVariableDeclaration(node: AstNode): boolean;
  isStringLiteral(node: AstNode): boolean;
}

type Classification =
  | "shared reached"
  | "named nonshared"
  | "parser-or-context refusal"
  | "annotation";
interface ClassificationRow {
  readonly id: string;
  readonly classification: Classification;
  readonly context: string;
}
interface ApplyEntry {
  readonly spelling: string;
  readonly arity: number;
  readonly positions: readonly number[];
}
export interface InventoryPosition {
  readonly constructor: string;
  readonly spelling: string;
  readonly position: string;
  readonly arity?: number;
}

const directory = "fixtures/tlc-operator-sharing";
const paths = [
  "node_modules/tla-ts/src/lexer.ts",
  "node_modules/tla-ts/src/parser.ts",
  "node_modules/tla-ts/src/ast.ts",
  "packages/idealis/src/tla-safety.ts",
  "crates/rtla/src/lexer.rs",
  "crates/rtla/src/parser.rs",
  "crates/rtla/src/ast.rs",
  "crates/rtla-compiler/src/lib.rs",
  "crates/rtla-compiler/src/typed.rs",
  "crates/rtla-compiler/src/typed_lower.rs",
  "crates/rtla-compiler/src/typed_expr.rs",
] as const;
const api = createRequire(import.meta.resolve("tla-ts/package.json"))(
  "@typescript/typescript6",
) as TypeScriptApi;
const sorted = (values: Iterable<string>) => [...new Set(values)].sort();
const walk = (node: AstNode, visit: (node: AstNode) => void): void => {
  visit(node);
  node.forEachChild((child) => walk(child, visit));
};
const parse = (name: string, source: string) =>
  api.createSourceFile(name, source, api.ScriptTarget.Latest, true);
const literalStrings = (node: AstNode) => {
  const strings: string[] = [];
  walk(node, (child) => {
    if (api.isStringLiteral(child)) {
      assert.equal(typeof child.text, "string");
      strings.push(child.text as string);
    }
  });
  return strings;
};
const namedInitializers = (source: AstNode) => {
  const result = new Map<string, AstNode>();
  walk(source, (node) => {
    if (api.isVariableDeclaration(node) && node.name && node.initializer)
      result.set(node.name.getText(source), node.initializer);
  });
  return result;
};
const section = (source: string, start: string, end: string) => {
  const begin = source.indexOf(start);
  assert.ok(begin >= 0, `missing source section ${start}`);
  const finish = source.indexOf(end, begin + start.length);
  assert.ok(finish > begin, `missing source section end ${end}`);
  return source.slice(begin, finish);
};
const quoted = (source: string): string[] =>
  [...source.matchAll(/"(?:\\.|[^"\\])*"/g)].map(
    (match) => JSON.parse(match[0]) as string,
  );
const pairs = (source: string): readonly (readonly [string, string])[] =>
  [
    ...source.matchAll(
      /\(\s*("(?:\\.|[^"\\])*")\s*,\s*("(?:\\.|[^"\\])*")\s*\)/g,
    ),
  ].map(
    (match) =>
      [
        JSON.parse(match[1] as string) as string,
        JSON.parse(match[2] as string) as string,
      ] as const,
  );
const balanced = (source: string, opening: number) => {
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let index = opening; index < source.length; index++) {
    const value = source[index];
    if (inString) {
      if (escaped) escaped = false;
      else if (value === "\\") escaped = true;
      else if (value === '"') inString = false;
    } else if (value === '"') inString = true;
    else if (value === "{") depth++;
    else if (value === "}" && --depth === 0)
      return source.slice(opening + 1, index);
  }
  assert.fail("unbalanced Rust AST declaration");
};
const rustDeclarations = (source: string) =>
  Object.fromEntries(
    [...source.matchAll(/pub (?:enum|struct) (\w+)[^{;]*\{/g)].map((match) => {
      const body = balanced(source, (match.index ?? 0) + match[0].length - 1);
      return [match[1] as string, body.replace(/\s+/g, " ").trim()];
    }),
  );
const syntaxSpellings = [
  "Cardinality",
  "Len",
  "Head",
  "Tail",
  "Seq",
  "Append",
  "SubSeq",
  "ApaFoldSet",
  "ApaFoldSeqLeft",
  "Variant",
  "VariantTag",
  "VariantGetUnsafe",
];

export const extractAnalysisInventory = async () => {
  const sources = Object.fromEntries(
    await Promise.all(
      paths.map(async (path) => [path, await readFile(path, "utf8")] as const),
    ),
  );
  const get = (path: (typeof paths)[number]) => {
    const source = sources[path];
    assert.equal(typeof source, "string");
    return source as string;
  };
  const tsLexer = parse("lexer.ts", get(paths[0]));
  const tsParser = parse("parser.ts", get(paths[1]));
  const tsAst = parse("ast.ts", get(paths[2]));
  const tables = new Map([
    ...namedInitializers(tsLexer),
    ...namedInitializers(tsParser),
  ]);
  const table = (name: string) => {
    const initializer = tables.get(name);
    assert.ok(initializer, name);
    return literalStrings(initializer);
  };
  const aliasTable = (name: string, backslash = false) => {
    const strings = table(name);
    assert.equal(strings.length % 2, 0, name);
    return Array.from(
      { length: strings.length / 2 },
      (_, index) =>
        [
          `${backslash ? "\\" : ""}${strings[index * 2] as string}`,
          strings[index * 2 + 1] as string,
        ] as const,
    );
  };
  const tsAliases = [
    ...aliasTable("BACKSLASH_OPERATORS", true),
    ...aliasTable("SYMBOL_ALIASES"),
    ...aliasTable("UNICODE_OPERATORS"),
  ];
  const tsInfix = sorted([...table("RELATIONS"), ...table("INFIX_OPERATORS")]);
  const tsPostfix = sorted(table("POSTFIX_OPERATORS"));
  const tsSymbols = sorted(table("SYMBOL_OPERATORS"));
  const tsPrefix = sorted([
    ...table("DEFINABLE_PREFIX_OPERATORS").filter((name) => name !== "-."),
    "-",
  ]);
  const tsDeclarations: Record<string, unknown> = {};
  const interfaces = new Map<string, AstNode>();
  let expressionUnion: AstNode | undefined;
  walk(tsAst, (node) => {
    if (api.isInterfaceDeclaration(node) && node.name) {
      const name = node.name.getText(tsAst);
      interfaces.set(name, node);
      tsDeclarations[name] = (node.members ?? []).map((member) => ({
        field: member.name?.getText(tsAst) ?? "",
        optional: member.questionToken !== undefined,
        type: member.type?.getText(tsAst).replace(/\s+/g, " ") ?? "",
      }));
    }
    if (api.isTypeAliasDeclaration(node) && node.name) {
      const name = node.name.getText(tsAst);
      tsDeclarations[name] =
        node.type?.getText(tsAst).replace(/\s+/g, " ") ?? "";
      if (name === "Expression") expressionUnion = node.type;
    }
  });
  assert.ok(expressionUnion);
  const tsConstructors = sorted(
    (expressionUnion.types ?? []).flatMap((node) => {
      const declaration = interfaces.get(node.getText(tsAst));
      assert.ok(declaration, node.getText(tsAst));
      const kind = declaration.members?.find(
        (member) => member.name?.getText(tsAst) === "kind",
      );
      assert.ok(kind?.type);
      return literalStrings(kind.type);
    }),
  );
  const rsLexer = get(paths[4]);
  const rsParser = get(paths[5]);
  const rsAst = get(paths[6]);
  const rustInfix = sorted(
    quoted(section(rsParser, "fn infix(", "\nfn within_ast_limit(")),
  );
  const rustAliases = [
    ...pairs(section(rsLexer, "const ALIASES:", "\nstruct Scanner")),
    ["/=", "#"] as const,
    ["=<", "<="] as const,
  ];
  const rustSymbols = sorted(
    quoted(section(rsLexer, "const SYMBOLS:", "const ALIASES:")),
  );
  const rustAstBody = balanced(
    rsAst,
    rsAst.indexOf("{", rsAst.indexOf("pub enum ExpressionKind")),
  );
  const rustConstructors = sorted(
    [...rustAstBody.matchAll(/^ {4}(\w+)(?: \{|,)/gm)].map(
      (match) => match[1] as string,
    ),
  );
  const tsSpellings = sorted([
    ...tsInfix,
    ...tsPostfix,
    ...tsPrefix,
    ...syntaxSpellings,
    ...tsAliases.map(([raw]) => raw),
    "'",
  ]);
  const rustPrefix = [
    "~",
    "-",
    "[]",
    "<>",
    "ENABLED",
    "UNCHANGED",
    "SUBSET",
    "UNION",
    "DOMAIN",
    "/\\",
    "\\/",
  ];
  const rustPostfix = ["'", "^+", "^*", "^#"];
  const rustSpellings = sorted([
    ...rustInfix,
    ...rustPrefix,
    ...rustPostfix,
    ...syntaxSpellings,
    ...rustAliases.map(([raw]) => raw),
  ]);
  const tsAliasMap = new Map(tsAliases);
  const rustAliasMap = new Map(rustAliases);
  const entries = [
    ...tsSpellings.map((spelling) => ({
      frontend: "TS",
      spelling,
      canonical: tsAliasMap.get(spelling) ?? spelling,
    })),
    ...rustSpellings.map((spelling) => ({
      frontend: "Rust",
      spelling,
      canonical: rustAliasMap.get(spelling) ?? spelling,
    })),
  ];
  const tsLower = parse("tla-safety.ts", get(paths[3]));
  const rustLower = paths.slice(7).map(get).join("\n");
  const lowerEvidence = {
    TS: sorted(literalStrings(tsLower)),
    Rust: sorted(quoted(rustLower)),
    rustConstructors: sorted(
      [...rustLower.matchAll(/E::(\w+)/g)].map((match) => match[1] as string),
    ),
    rustFunctions: sorted(
      [...rustLower.matchAll(/\bfn (\w+)\s*\(/g)].map(
        (match) => match[1] as string,
      ),
    ),
  };
  return {
    sourceSha256: Object.fromEntries(
      paths.map((path) => [
        path,
        createHash("sha256").update(get(path)).digest("hex"),
      ]),
    ),
    parser: {
      tsInfix,
      tsPostfix,
      tsPrefix,
      tsAliases,
      tsSymbols,
      rustInfix,
      rustPrefix,
      rustPostfix,
      rustAliases,
      rustSymbols,
    },
    ast: {
      tsConstructors,
      rustConstructors,
      tsDeclarations,
      rustDeclarations: rustDeclarations(rsAst),
    },
    entries,
    lowerEvidence,
  };
};

export const inventoryDifference = (
  expected: readonly string[],
  actual: readonly string[],
) => ({
  missing: sorted(expected.filter((id) => !actual.includes(id))),
  extra: sorted(actual.filter((id) => !expected.includes(id))),
});
const noDifference = (
  expected: readonly string[],
  actual: readonly string[],
  label: string,
) => {
  assert.equal(
    new Set(actual).size,
    actual.length,
    `${label}: duplicate entries`,
  );
  assert.deepEqual(
    inventoryDifference(expected, actual),
    { missing: [], extra: [] },
    `${label}: two-way difference`,
  );
};

export const verifyAnalysisInventory = async (
  positions: readonly InventoryPosition[],
) => {
  const manifest = JSON.parse(
    await readFile(`${directory}/analysis-inventory.json`, "utf8"),
  ) as {
    readonly extracted: Awaited<ReturnType<typeof extractAnalysisInventory>>;
    readonly classifications: readonly ClassificationRow[];
    readonly sharedApplyArityEntries: readonly ApplyEntry[];
    readonly sharedNonApplyPositions: readonly InventoryPosition[];
    readonly syntaxContexts: readonly {
      readonly id: string;
      readonly lowering: readonly { readonly context: string }[];
    }[];
    readonly nonfixContexts: readonly { readonly spelling: string }[];
  };
  const extracted = await extractAnalysisInventory();
  assert.deepEqual(
    extracted.sourceSha256,
    manifest.extracted.sourceSha256,
    "analysis inventory source digest changed",
  );
  assert.deepEqual(
    extracted,
    manifest.extracted,
    "analysis inventory mechanical re-extraction changed",
  );
  const entryIds = extracted.entries.map(
    ({ frontend, spelling }) => `spelling/${frontend}/${spelling}`,
  );
  const constructorIds = [
    ...extracted.ast.tsConstructors.map((kind) => `constructor/TS/${kind}`),
    ...extracted.ast.rustConstructors.map((kind) => `constructor/Rust/${kind}`),
  ];
  const lexicalIds = [
    ...extracted.parser.tsSymbols.map((symbol) => `lexical/TS/${symbol}`),
    ...extracted.parser.rustSymbols.map((symbol) => `lexical/Rust/${symbol}`),
  ];
  const contextIds = [
    ...manifest.syntaxContexts.flatMap((row) =>
      row.lowering.map(({ context }) => `context/syntax/${row.id}/${context}`),
    ),
    ...manifest.nonfixContexts.map(
      (context) => `context/nonfix/${context.spelling}`,
    ),
  ];
  const annotationIds = [
    "annotation/TS/Fairness.enabled",
    "annotation/TS/Let.instances",
    "annotation/TS/Instance.substitutions",
  ];
  const ids = [
    ...entryIds,
    ...constructorIds,
    ...lexicalIds,
    ...annotationIds,
    ...contextIds,
  ];
  noDifference(
    ids,
    manifest.classifications.map((row) => row.id),
    "exclusive parser/lower inventory classification",
  );
  for (const row of manifest.classifications) {
    assert.ok(
      [
        "shared reached",
        "named nonshared",
        "parser-or-context refusal",
        "annotation",
      ].includes(row.classification),
      row.id,
    );
    assert.ok(row.context.length > 0, row.id);
  }
  assert.equal(manifest.sharedApplyArityEntries.length, 56);
  assert.equal(
    manifest.sharedApplyArityEntries.reduce(
      (count, entry) => count + entry.positions.length,
      0,
    ),
    102,
  );
  const key = (spelling: string, arity: number, position: string) =>
    `${spelling}/${arity}/${position}`;
  const expected = manifest.sharedApplyArityEntries.flatMap((entry) =>
    entry.positions.map((position) =>
      key(entry.spelling, entry.arity, `args[${position}]`),
    ),
  );
  const actual = positions
    .filter((position) => position.constructor === "Apply")
    .map((position) => {
      assert.equal(typeof position.arity, "number", position.spelling);
      return key(
        position.spelling === "user call"
          ? "user call representative"
          : position.spelling,
        position.arity as number,
        position.position,
      );
    });
  noDifference(
    expected,
    actual,
    "shared spelling/arity-position versus generated IDs",
  );
  const aliases = new Map<string, readonly string[]>([
    ["And", ["\\land", "∧"]],
    ["Or", ["\\lor", "∨"]],
    ["Forall", ["\\forall", "∀"]],
    ["Exists", ["\\exists", "∃"]],
    ["Always", ["□"]],
    ["Eventually", ["◇"]],
    ["Tuple", ["⟨"]],
    ["AngleAction", ["⟨"]],
  ]);
  const nonApplyId = (position: InventoryPosition) => {
    const alias = aliases
      .get(position.constructor)
      ?.find((value) => position.spelling.includes(value));
    const path = position.position.replace(/\.Index(?=\.args)/g, "");
    return `${position.constructor}/${alias ?? "<canonical>"}/${path}`;
  };
  assert.equal(manifest.sharedNonApplyPositions.length, 96);
  noDifference(
    manifest.sharedNonApplyPositions.map(nonApplyId),
    positions
      .filter((position) => position.constructor !== "Apply")
      .map(nonApplyId),
    "shared constructor/alias-position versus generated IDs",
  );
  const sharedSpellings = new Set(
    manifest.classifications
      .filter(
        (row) =>
          row.classification === "shared reached" &&
          row.id.startsWith("spelling/"),
      )
      .map((row) => row.id.split("/").slice(2).join("/")),
  );
  for (const entry of manifest.sharedApplyArityEntries) {
    if (entry.spelling !== "user call representative")
      assert.ok(sharedSpellings.has(entry.spelling), entry.spelling);
  }
};
