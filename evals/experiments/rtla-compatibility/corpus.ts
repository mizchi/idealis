import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { join, relative } from "node:path";

export interface Input {
  readonly name: string;
  readonly family: string;
  readonly source: string;
  readonly expected: "accepted" | "syntax";
  readonly expectedStable?: "accepted" | "syntax";
  readonly attributes: readonly string[];
  readonly expectedAst: string;
  readonly origin: string;
}

export const corpus = async (upstream: string): Promise<readonly Input[]> => {
  const root = join(
    upstream,
    "tlatools/org.lamport.tlatools/test/tla2sany/corpus",
  );
  const files = execFileSync("rg", ["--files", root])
    .toString()
    .trim()
    .split("\n")
    .filter((path) => path.endsWith(".txt"))
    .sort();
  const inputs: Input[] = [];
  for (const path of files) {
    const content = await readFile(path, "utf8");
    const headers = [
      ...content.matchAll(
        /^===+\|\|\|\r?\n(.*?)(?=\r?\n===+\|\|\|)\r?\n===+\|\|\|\r?\n/gms,
      ),
    ];
    assert.ok(headers.length, path);
    for (const [index, header] of headers.entries()) {
      const body = content.slice(
        header.index + header[0].length,
        headers[index + 1]?.index,
      );
      const separator = /^---+\|\|\|\r?\n/m.exec(body);
      assert.ok(separator && header[1], path);
      const lines = header[1].split(/\r?\n/);
      const attributes = lines.filter((line) => line.startsWith(":"));
      assert.ok(
        attributes.every((attribute) =>
          [":error", ":skip"].includes(attribute),
        ),
        header[1],
      );
      inputs.push({
        name: lines
          .filter((line) => !line.startsWith(":"))
          .join("\n")
          .trim(),
        family: relative(root, path),
        source: body.slice(0, separator.index),
        expected: attributes.includes(":error") ? "syntax" : "accepted",
        attributes,
        expectedAst: body.slice(separator.index + separator[0].length),
        origin: relative(upstream, path),
      });
    }
  }
  return inputs;
};

export const focus: readonly Input[] = [
  ["subset-range", "SUBSET 1..2", "accepted"],
  ["union-range", "UNION a..b", "accepted"],
  ["domain-add", "DOMAIN f + n", "accepted"],
  ["subset-product", "SUBSET S \\X T", "accepted"],
  ["union-product", "UNION S \\X T", "accepted"],
  ["domain-product", "DOMAIN f \\X T", "accepted"],
  ["subset-union-conflict", "SUBSET S \\cup T", "syntax"],
  ["union-union-conflict", "UNION S \\cup T", "syntax"],
  ["domain-range-conflict", "DOMAIN f .. n", "syntax"],
  ["subset-parenthesized", "(SUBSET S) \\cup T", "accepted"],
  ["union-parenthesized", "(UNION S) \\cup T", "accepted"],
  ["domain-parenthesized", "(DOMAIN f) .. n", "accepted"],
  ["subset-prime", "SUBSET S'", "accepted"],
].map(([name, expression, expected]) => {
  assert.ok(
    name && expression && (expected === "accepted" || expected === "syntax"),
  );
  return {
    name,
    family: "prefix-focus",
    source: `---- MODULE Test ----\nExpr == ${expression}\n====`,
    expected,
    expectedStable: [
      "domain-add",
      "subset-product",
      "union-product",
      "domain-product",
    ].includes(name)
      ? "syntax"
      : "accepted",
    attributes: [],
    expectedAst: "",
    origin: "SANY Operators.java: SUBSET/UNION 8..8, DOMAIN 9..9",
  };
});
