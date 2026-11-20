import { source } from "./corpus.js";

export interface ModelCase {
  readonly name: string;
  readonly family: "graph";
  readonly source: string;
  readonly config: string;
  readonly expected:
    | "complete"
    | "invariant-violation"
    | "builtin-conflict"
    | "import-ambiguity";
}

export const models: readonly ModelCase[] = [
  ...[
    ["infix", "left %% right == ~left", "x %% TRUE", ""],
    ["prefix", "-. value == ~value", "-x", ""],
    ["postfix", "value ^+ == ~value", "x^+", ""],
    ["nonfix", "left %% right == ~left", "%%(x, TRUE)", ""],
    ["let", "", "LET left %% right == ~left IN x %% TRUE", ""],
    ["local-import", "left + right == ~left", "x + TRUE", "EXTENDS Sequences"],
  ].flatMap(([name, definition, next, extendsLine]) =>
    [false, true].map((broken) => ({
      name: `${name}-${broken ? "fault" : "safe"}`,
      family: "graph" as const,
      source: source(
        `${extendsLine}\nVARIABLE x\n${definition}\nInit == x = FALSE\nNext == x' = (${next})\nSafe == ${broken ? "x = FALSE" : "x \\in BOOLEAN"}`,
      ).replace("MODULE Symbolic", "MODULE Differential"),
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
      expected: broken
        ? ("invariant-violation" as const)
        : ("complete" as const),
    })),
  ),
  ...[
    ["builtin-and", "", "left /\\ right == FALSE", "builtin-conflict"],
    ["builtin-domain", "", "DOMAIN value == value", "builtin-conflict"],
    ["builtin-prime", "", "value' == value", "builtin-conflict"],
    ["builtin-product", "", "left \\X right == left", "builtin-conflict"],
    [
      "unused-nested-builtin",
      "",
      "Unused == LET left /\\ right == FALSE IN TRUE",
      "builtin-conflict",
    ],
    ["builtin-formal", "", "Unused(_ /\\ _) == TRUE", "builtin-conflict"],
    [
      "import-plus",
      "EXTENDS Naturals",
      "left + right == left",
      "import-ambiguity",
    ],
    [
      "import-negative",
      "EXTENDS Integers",
      "-. value == value",
      "import-ambiguity",
    ],
    [
      "import-concat",
      "EXTENDS Sequences",
      "left \\o right == left",
      "import-ambiguity",
    ],
    [
      "import-override",
      "EXTENDS TLC",
      "left @@ right == left",
      "import-ambiguity",
    ],
  ].map(([name, extendsLine, definition, expected]): ModelCase => {
    if (
      !name ||
      (expected !== "builtin-conflict" && expected !== "import-ambiguity")
    )
      throw Error("Invalid model control");
    return {
      name,
      family: "graph" as const,
      source: source(
        `${extendsLine}\nVARIABLE x\n${definition}\nInit == x = 0\nNext == UNCHANGED x`,
      ).replace("MODULE Symbolic", "MODULE Differential"),
      config: "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE",
      expected,
    };
  }),
];
