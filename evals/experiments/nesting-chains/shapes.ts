import { entry, parenthesized, rightNested, type Source } from "./sources.js";

type Tree = null | readonly [Tree, Tree];

const trees = (leaves: number): readonly Tree[] =>
  leaves === 1
    ? [null]
    : Array.from({ length: leaves - 1 }, (_, split) => split + 1).flatMap(
        (left) =>
          trees(left).flatMap((first) =>
            trees(leaves - left).map((second): Tree => [first, second]),
          ),
      );

const render = (
  tree: Tree,
  operators: readonly string[],
  leaves: readonly string[],
  counters: { operator: number; leaf: number },
  conditions: readonly string[],
): string => {
  if (tree === null) {
    const leaf = leaves[counters.leaf % leaves.length] ?? "TRUE";
    counters.leaf += 1;
    return leaf;
  }
  const operator = operators[counters.operator % operators.length] ?? "/\\";
  counters.operator += 1;
  const left = render(tree[0], operators, leaves, counters, conditions);
  const right = render(tree[1], operators, leaves, counters, conditions);
  if (operator === "IF") {
    const condition = conditions[(counters.operator - 1) % conditions.length];
    return `(IF ${condition} THEN ${left} ELSE ${right})`;
  }
  return `(${left} ${operator} ${right})`;
};

const combinations = (size: number, choices: readonly string[]) =>
  Array.from({ length: size }, () => choices).reduce<
    readonly (readonly string[])[]
  >(
    (rows, options) =>
      rows.flatMap((row) => options.map((option) => [...row, option])),
    [[]],
  );

const enumerate = (
  prefix: string,
  group: string,
  maximum: number,
  choices: readonly string[],
  leaves: readonly string[],
  conditions: readonly string[],
  body: (shape: string) => string,
  config: string,
): readonly Source[] =>
  Array.from({ length: maximum - 1 }, (_, index) => index + 2).flatMap((size) =>
    trees(size).flatMap((tree, shape) =>
      combinations(size - 1, choices).map((operators, variant) =>
        entry(
          `${prefix}-${size}-${shape}-${variant}`,
          group,
          body(
            render(
              tree,
              operators,
              leaves,
              { operator: 0, leaf: 0 },
              conditions,
            ),
          ),
          config,
          null,
        ),
      ),
    ),
  );

const invariantConfig =
  "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n";
const counter = "Init == x = 0\nNext == x' = IF x < 5 THEN x + 1 ELSE 0\n";

const expansions = (): readonly Source[] =>
  [6, 12, 13].flatMap((count) => {
    const factors = Array.from(
      { length: count },
      (_, index) => `(x < ${100 + index} \\/ x < ${200 + index})`,
    );
    const balanced = (items: readonly string[]): string =>
      items.length === 1
        ? (items[0] ?? "TRUE")
        : `(${balanced(items.slice(0, items.length >> 1))} /\\ ${balanced(items.slice(items.length >> 1))})`;
    const arrangements: readonly (readonly [string, string])[] = [
      ["flat", factors.join(" /\\ ")],
      ["left", parenthesized("/\\", factors)],
      ["right", rightNested("/\\", factors)],
      ["balanced", balanced(factors)],
    ];
    return arrangements.map(([arrangement, tree]) =>
      entry(
        `expansion-${arrangement}-${count}`,
        "shape",
        `Init == x = 0\nNext == (${tree}) /\\ x' = 0\nInv == x >= 0\n`,
        invariantConfig,
        null,
      ),
    );
  });

export const shapes = (): readonly Source[] => [
  ...expansions(),
  ...enumerate(
    "action",
    "shape",
    4,
    ["/\\", "\\/", "IF"],
    [
      "x < 5",
      "(\\E k \\in 0..1 : x' = k)",
      "(x' = 2 \\/ x' = 3)",
      "(\\E j \\in 0..2 : x' = j)",
      "(LET d == x + 1 IN x' = d)",
    ],
    ["x < 3", "x = 0", "x /= 2"],
    (shape) => `Init == x = 0\nNext == ${shape}\nInv == x >= 0\n`,
    invariantConfig,
  ),
  ...enumerate(
    "action-wide",
    "shape",
    5,
    ["/\\", "\\/"],
    [
      "x < 5",
      "(\\E k \\in 0..1 : x' = k)",
      "(x' = 2 \\/ x' = 3)",
      "(\\E j \\in 0..2 : x' = j)",
      "(LET d == x + 1 IN x' = d)",
    ],
    [],
    (shape) => `Init == x = 0\nNext == ${shape}\nInv == x >= 0\n`,
    invariantConfig,
  ),
  ...enumerate(
    "expression",
    "shape",
    4,
    ["/\\", "\\/", "IF"],
    ["x >= 0", "x < 100", "x /= 7", "(x = 1 \\/ x = 2)", "~(x = 9)"],
    ["x < 3", "x = 0", "x /= 2"],
    (shape) => `${counter}Inv == ${shape}\n`,
    invariantConfig,
  ),
  ...enumerate(
    "expression-wide",
    "shape",
    5,
    ["/\\", "\\/"],
    ["x >= 0", "x < 100", "x /= 7", "(x = 1 \\/ x = 2)", "~(x = 9)"],
    [],
    (shape) => `${counter}Inv == ${shape}\n`,
    invariantConfig,
  ),
  ...enumerate(
    "specification",
    "shape",
    4,
    ["/\\"],
    ["x = 0", "x >= 0", "x < 100", "[][Next]_x"],
    [],
    (shape) =>
      `Next == x' = IF x < 5 THEN x + 1 ELSE 0\nSpec == ${shape}\nInv == x >= 0\n`,
    "SPECIFICATION Spec\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
  ),
  ...enumerate(
    "membership",
    "shape",
    4,
    ["IF"],
    ["{0}", "{1}", "0..5", "{0, 1}"],
    ["x = 0", "x < 3", "x /= 2"],
    (shape) => `${counter}Types == ${shape}\nInv == x \\in Types\n`,
    invariantConfig,
  ),
];
