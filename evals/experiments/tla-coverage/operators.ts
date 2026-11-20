import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";

const upstream = process.argv[2];
assert.ok(upstream);
const path = "tlatools/org.lamport.tlatools/src/tla2sany/parser/Operators.java";
const source = execFileSync("git", ["show", `v1.8.0:${path}`], {
  cwd: upstream,
}).toString();
const strings = (text: string) =>
  [...text.matchAll(/"(?:\\.|[^"\\])*"/g)].map(
    (match) => JSON.parse(match[0]) as string,
  );
const names = new Map([
  ["\\lnot", "~"],
  ["\\land", "/\\"],
  ["\\lor", "\\/"],
  ["\\equiv", "<=>"],
  ["/=", "#"],
  ["\\leq", "<="],
  ["\\geq", ">="],
  ["\\times", "\\X"],
  ["\\intersect", "\\cap"],
  ["\\union", "\\cup"],
]);
const operators = [
  ...source.matchAll(
    /^\s*new Operator\(("(?:\\.|[^"\\])*"),\s*(\d+),\s*(\d+),\s*Operators\.assoc(\w+),\s*Operators\.(\w+)\)/gm,
  ),
].map((match) => {
  const raw: string = JSON.parse(match[1] ?? '""');
  const canonical = names.get(raw) ?? raw;
  return {
    spelling: raw,
    canonical,
    low: Number(match[2]) / 10,
    high: Number(match[3]) / 10,
    associative: match[4] === "Left",
    fix: match[5],
  };
});
assert.equal(operators.length, 101);
const synonyms = [
  ...source.matchAll(/^\s*new String\[\] \{([^}]+)\}/gm),
].flatMap((match) => {
  const values = strings(match[1] ?? "");
  const canonical = values[0];
  assert.ok(canonical);
  return values.map((spelling) => ({
    spelling,
    canonical: names.get(canonical) ?? canonical,
  }));
});
const aliases = new Map(
  synonyms.map((alias) => [alias.spelling, alias.canonical]),
);
for (const operator of operators)
  aliases.set(operator.spelling, operator.canonical);
for (const [spelling, canonical] of [
  ["\\forall", "\\A"],
  ["\\exists", "\\E"],
  ["∀", "\\A"],
  ["∃", "\\E"],
  ["⟨", "<<"],
  ["⟩_", ">>_"],
  ["⟩", ">>"],
  ["∷", "::"],
  ["≜", "=="],
  ["→", "->"],
  ["←", "<-"],
  ["↦", "|->"],
  ["ℕ", "Nat"],
  ["ℤ", "Int"],
  ["ℝ", "Real"],
]) {
  assert.ok(spelling && canonical);
  aliases.set(spelling, canonical);
}
const catalog = {
  reference:
    "tlaplus/tlaplus v1.8.0 Operators.java; legacy prefix SUBSET/UNION/DOMAIN 10..13 retained separately",
  operators,
  aliases: [...aliases].map(([spelling, canonical]) => ({
    spelling,
    canonical,
  })),
};
await writeFile(
  "crates/rtla/tests/fixtures/operators.json",
  `${JSON.stringify(catalog, null, 2)}\n`,
);
if (process.env.IDEALIS_GENERATE_OPERATORS === "1") {
  const lexer = await readFile("crates/rtla/src/lexer.rs", "utf8");
  const previous = lexer.slice(
    lexer.indexOf("const ALIASES:"),
    lexer.indexOf("\nstruct Scanner"),
  );
  for (const [spelling, canonical] of [
    ...previous.matchAll(
      /\(\s*("(?:\\.|[^"\\])*")\s*,\s*("(?:\\.|[^"\\])*")\s*\)/g,
    ),
  ].map(
    (match) =>
      [JSON.parse(match[1] ?? '""'), JSON.parse(match[2] ?? '""')] as const,
  ))
    aliases.set(spelling, canonical);
  const table = `const ALIASES: &[(&str, &str)] = &[\n${[...aliases]
    .filter(
      ([spelling]) => !/^\w+$/.test(spelling) && !["[", "."].includes(spelling),
    )
    .map(
      ([spelling, canonical]) =>
        `    (${JSON.stringify(spelling)}, ${JSON.stringify(canonical)}),`,
    )
    .join("\n")}\n];\n`;
  await writeFile(
    "crates/rtla/src/lexer.rs",
    lexer.replace(previous, () => table),
  );
  const parser = await readFile("crates/rtla/src/parser.rs", "utf8");
  const existing = [
    ...parser
      .slice(
        parser.indexOf("fn infix("),
        parser.indexOf("\nfn within_ast_limit("),
      )
      .matchAll(/"(?:\\.|[^"\\])*"/g),
  ].map((match) => JSON.parse(match[0]) as string);
  const additions = operators.filter(
    (operator) =>
      ["infix", "nfix"].includes(operator.fix ?? "") &&
      !existing.includes(operator.canonical) &&
      operator.canonical !== ".",
  );
  await writeFile(
    "crates/rtla/src/parser.rs",
    parser.replace(
      "        _ => return None,",
      () =>
        `${additions.map((operator) => `        ${JSON.stringify(operator.canonical)} => (${operator.low}, ${operator.high}, ${operator.associative}),`).join("\n")}\n        _ => return None,`,
    ),
  );
}
console.log({ operators: operators.length, aliases: catalog.aliases.length });
