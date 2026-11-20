export interface SymbolicCase {
  readonly id: string;
  readonly body: string;
  readonly accepted: boolean;
}

export const cases: readonly SymbolicCase[] = [
  {
    id: "infix-definition",
    body: "EXTENDS Naturals\na %% b == a + b\nMain == 2 %% 3",
    accepted: true,
  },
  {
    id: "prefix-definition",
    body: "-. value == IF value = 0 THEN 1 ELSE 0\nMain == -0",
    accepted: true,
  },
  {
    id: "postfix-definition",
    body: "value ^+ == value\nMain == 1^+",
    accepted: true,
  },
  {
    id: "local-let",
    body: "LOCAL a %% b == a\nMain == LET value ^+ == value IN (1 %% 2)^+",
    accepted: true,
  },
  {
    id: "symbolic-constants",
    body: "CONSTANT _ %% _, -. _, _ ^+",
    accepted: true,
  },
  {
    id: "symbolic-formals",
    body: "F(_ %% _, -. _, _ ^+) == TRUE",
    accepted: true,
  },
  {
    id: "operator-arguments",
    body: "Main == F(%%, -., ^+, +, -, ~, [], DOMAIN)",
    accepted: true,
  },
  { id: "qualified-infix", body: "Main == M!N!%%(1, 2)", accepted: true },
  { id: "qualified-postfix", body: "Main == M!^+(1)", accepted: true },
  { id: "qualified-prefix", body: "Main == M!~(TRUE)", accepted: true },
  { id: "nonfix-infix", body: "Main == %%(1, 2)", accepted: true },
  { id: "nonfix-prefix-minus", body: "Main == -.(1)", accepted: true },
  { id: "nonfix-plus", body: "Main == +(1, 2)", accepted: true },
  { id: "nonfix-arity-is-semantic", body: "Main == %%(1)", accepted: true },
  { id: "bad-prefix-arity", body: "Main == ~(TRUE, FALSE)", accepted: false },
  { id: "bare-operator-body", body: "Main == %%", accepted: true },
  { id: "bare-operator-set", body: "Main == {%%}", accepted: true },
  { id: "bare-operator-index", body: "Main == function[%%]", accepted: true },
  { id: "bad-minus-head", body: "- value == value", accepted: false },
  { id: "missing-infix-formal", body: "left %% == left", accepted: false },
  {
    id: "extra-postfix-formal",
    body: "left ^+ right == left",
    accepted: false,
  },
  { id: "missing-infix-placeholder", body: "CONSTANT _ %%", accepted: false },
  { id: "bad-minus-placeholder", body: "CONSTANT - _", accepted: false },
  { id: "variable-symbol", body: "VARIABLE _ %% _", accepted: false },
  { id: "variable-operator", body: "VARIABLE F(_)", accepted: false },
  { id: "product-definition", body: "left \\X right == left", accepted: true },
  { id: "product-reference", body: "Main == F(\\X)", accepted: false },
  { id: "qualified-product", body: "Main == M!\\X(1, 2)", accepted: true },
  { id: "string-head", body: 'left "%%" right == left', accepted: false },
  { id: "builtin-prefix-head", body: "DOMAIN value == value", accepted: true },
  { id: "builtin-infix-head", body: "left /\\ right == left", accepted: true },
  { id: "parenthesized-reference", body: "Main == F((+))", accepted: true },
  {
    id: "normal-function",
    body: "F(value) == value\nf[key \\in {1}] == F(key)\nMain == f[1]",
    accepted: true,
  },
];

export const source = (body: string): string =>
  `---- MODULE Symbolic ----\n${body}\n====\n`;
