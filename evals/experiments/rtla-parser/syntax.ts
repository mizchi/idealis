export const syntaxCases = [
  {
    name: "parenthesized-precedence",
    expression: "(1 = 2) = FALSE",
    expected: "module",
  },
  { name: "prefix-precedence", expression: "-(1 + 2) * 3", expected: "module" },
  {
    name: "nested-junction",
    expression: "/\\ TRUE\n    /\\ FALSE\n    /\\ TRUE\n/\\ TRUE",
    expected: "module",
  },
  {
    name: "aliases-and-based-numerals",
    expression: String.raw`\h10 \leq \o20 \land \b10 /= 3`,
    expected: "module",
  },
  {
    name: "case-arms",
    expression: "CASE x = 0 -> 1 [] OTHER -> 2",
    expected: "module",
  },
  {
    name: "nested-comments",
    expression: "(* a (* b *) c *) TRUE",
    expected: "module",
  },
  {
    name: "empty-tuple-and-set",
    expression: "<<{}, <<>>>>",
    expected: "module",
  },
  { name: "missing-bracket", expression: "[a |-> 1", expected: "syntax" },
  {
    name: "missing-comment-end",
    expression: "(* never closed",
    expected: "syntax",
  },
  {
    name: "missing-string-end",
    expression: '"never closed',
    expected: "syntax",
  },
  { name: "relational-chain", expression: "1 = 2 = 3", expected: "syntax" },
  { name: "mixed-associativity", expression: "1 / 2 * 3", expected: "syntax" },
  {
    name: "mixed-junctions",
    expression: "TRUE /\\ FALSE \\/ TRUE",
    expected: "syntax",
  },
  { name: "empty-call", expression: "F()", expected: "syntax" },
  { name: "empty-index", expression: "f[]", expected: "syntax" },
  { name: "missing-comma", expression: "{1 2}", expected: "syntax" },
  { name: "offside-body", expression: "/\\\nTRUE", expected: "syntax" },
  {
    name: "lambda-argument",
    expression: "Map(LAMBDA a: a, S)",
    expected: "module",
  },
].map((entry) => ({
  name: entry.name,
  source: `---- MODULE Syntax ----\nVARIABLE x\nExpr ==\n${entry.expression
    .split("\n")
    .map((line) => `  ${line}`)
    .join("\n")}\n====`,
  expected: entry.expected,
}));
