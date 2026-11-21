export interface Source {
  readonly name: string;
  readonly group: string;
  readonly source: string;
  readonly config: string;
  readonly equivalent: string | null;
  readonly semantic: string | null;
}

export const lengths = [
  2, 3, 4, 24, 25, 38, 39, 40, 44, 47, 48, 49, 54, 61, 62,
] as const;
export const positions = [1, 22, 23, 44] as const;
export const controlLength = 44;
export const boundaryDepths = [38, 39, 40, 41] as const;

const indices = (count: number): readonly number[] =>
  Array.from({ length: count }, (_, index) => index);
const module = (body: string): string =>
  `---- MODULE NestingChains ----\nEXTENDS Integers\nVARIABLE x\n${body}====\n`;
const counter = (limit: number): string =>
  `Init == x = 0\nNext == x' = IF x < ${limit} THEN x + 1 ELSE 0\n`;
const invariantConfig =
  "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n";
const specificationConfig =
  "SPECIFICATION Spec\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n";
const propertyConfig =
  "INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n";

export const parenthesized = (
  operator: string,
  operands: readonly string[],
): string =>
  operands
    .slice(1)
    .reduce(
      (left, operand) => `(${left} ${operator} ${operand})`,
      operands[0] ?? "TRUE",
    );
export const flat = (operator: string, operands: readonly string[]): string =>
  operands.join(` ${operator} `);
export const rightNested = (
  operator: string,
  operands: readonly string[],
): string =>
  operands
    .slice(0, -1)
    .reduceRight(
      (right, operand) => `(${operand} ${operator} ${right})`,
      operands[operands.length - 1] ?? "TRUE",
    );
const bulleted = (operator: string, operands: readonly string[]): string =>
  `\n${operands.map((operand) => `  ${operator} ${operand}`).join("\n")}\n`;
export const elseIf = (
  conditions: readonly string[],
  values: readonly string[],
  otherwise: string,
): string =>
  `${conditions.map((condition, index) => `IF ${condition} THEN ${values[index] ?? ""} ELSE `).join("")}${otherwise}`;
export const caseOther = (
  conditions: readonly string[],
  values: readonly string[],
  otherwise: string,
): string =>
  `CASE ${conditions.map((condition, index) => `${condition} -> ${values[index] ?? ""}`).join(" [] ")} [] OTHER -> ${otherwise}`;

const conjunctionTerms = (count: number): readonly string[] =>
  indices(count).map((index) => `x /= ${200 + index}`);
const disjunctionTerms = (count: number): readonly string[] =>
  indices(count).map((index) =>
    index === count - 1 ? "x >= 0" : `x = ${200 + index}`,
  );
const stepArms = (count: number) => ({
  conditions: indices(count).map((index) => `x = ${index}`),
  values: indices(count).map((index) => String(100 + index)),
});

export const entry = (
  name: string,
  group: string,
  body: string,
  config: string,
  equivalent: string | null,
  semantic: string | null = null,
): Source => ({
  name,
  group,
  source: module(body),
  config,
  equivalent,
  semantic,
});

const lengthSources = (): readonly Source[] =>
  lengths.flatMap((length): readonly Source[] => {
    const walk = counter(5);
    const jump = counter(length + 1);
    const { conditions, values } = stepArms(length);
    const label = (expression: string) =>
      `label == ${expression}\nInv == label >= 0\n`;
    const row = (form: string, body: string, equivalent: string | null) =>
      entry(
        `${form}-${length}`,
        "length",
        body,
        invariantConfig,
        equivalent === null ? null : `${equivalent}-${length}`,
      );
    return [
      row(
        "paren-and",
        `${walk}Inv == ${parenthesized("/\\", conjunctionTerms(length))}\n`,
        "flat-and",
      ),
      row(
        "flat-and",
        `${walk}Inv == ${flat("/\\", conjunctionTerms(length))}\n`,
        null,
      ),
      row(
        "paren-or",
        `${walk}Inv == ${parenthesized("\\/", disjunctionTerms(length))}\n`,
        "flat-or",
      ),
      row(
        "flat-or",
        `${walk}Inv == ${flat("\\/", disjunctionTerms(length))}\n`,
        null,
      ),
      row(
        "if-chain",
        `${jump}${label(elseIf(conditions, values, "0"))}`,
        "case-other",
      ),
      row(
        "case-other",
        `${jump}${label(caseOther(conditions, values, "0"))}`,
        null,
      ),
    ];
  });

const range = indices(controlLength);
const walkLong = counter(49);

interface Poison {
  readonly family: string;
  readonly falseAt: string;
  readonly skipped: string;
  readonly term: string;
  readonly value: string;
  readonly condition: string;
  readonly trailing: string;
}
const poisons: readonly Poison[] = [
  {
    family: "boolean",
    falseAt: "x /= 0",
    skipped: "1",
    term: "1",
    value: "(1 + TRUE)",
    condition: "5",
    trailing: "x /= 0",
  },
  {
    family: "division",
    falseAt: "x /= 3",
    skipped: "10 \\div (3 - x) /= 0",
    term: "10 \\div (3 - x) /= 0",
    value: "(1 \\div 0)",
    condition: "(1 \\div 0) = 0",
    trailing: "x /= 3",
  },
];

const replaced = (
  operands: readonly string[],
  position: number,
  replacement: string,
) =>
  operands.map((operand, index) =>
    index === position - 1 ? replacement : operand,
  );

const pair = (
  name: string,
  chain: string,
  alternative: string,
  chainBody: string,
  alternativeBody: string,
): readonly Source[] => [
  entry(
    `${name}-${chain}`,
    "semantics",
    chainBody,
    invariantConfig,
    `${name}-${alternative}`,
  ),
  entry(
    `${name}-${alternative}`,
    "semantics",
    alternativeBody,
    invariantConfig,
    null,
  ),
];

const operandPair = (
  name: string,
  operands: readonly string[],
  operator: string,
): readonly Source[] =>
  pair(
    name,
    "paren",
    "flat",
    `${walkLong}Inv == ${parenthesized(operator, operands)}\n`,
    `${walkLong}Inv == ${flat(operator, operands)}\n`,
  );

const alternativePair = (
  name: string,
  conditions: readonly string[],
  values: readonly string[],
  invariant: string,
): readonly Source[] => {
  const body = (expression: string) =>
    `${walkLong}label == ${expression}\nInv == ${invariant}\n`;
  return pair(
    name,
    "if",
    "case",
    body(elseIf(conditions, values, "8")),
    body(caseOther(conditions, values, "8")),
  );
};

const semanticSources = (): readonly Source[] =>
  poisons.flatMap((poison) =>
    positions.flatMap((position): readonly Source[] => {
      const last = position === controlLength;
      const tag = (kind: string) => `${poison.family}-${kind}-${position}`;
      const truths = conjunctionTerms(controlLength);
      const unreachable = range.map((index) => `x = ${1000 + index}`);
      const plain = range.map((index) => String(100 + index));
      const poisoned = range.map(() => poison.value);
      const following = (operands: readonly string[], term: string) =>
        operands.map((operand, index) => (index === position ? term : operand));
      const disjunction = range.map((index) =>
        index === position - 1
          ? "x >= 0"
          : index < position - 1
            ? `x = ${300 + index}`
            : poison.term,
      );
      return [
        ...(poison.family === "boolean"
          ? operandPair(
              tag("and-false"),
              replaced(truths, position, "x /= 3"),
              "/\\",
            )
          : []),
        ...(last
          ? []
          : operandPair(
              tag("and-false-skips"),
              following(
                replaced(truths, position, poison.falseAt),
                poison.skipped,
              ),
              "/\\",
            )),
        ...operandPair(
          tag("and-error"),
          replaced(truths, position, poison.term),
          "/\\",
        ),
        ...(last
          ? []
          : operandPair(
              tag("and-error-first"),
              following(
                replaced(truths, position, poison.term),
                poison.trailing,
              ),
              "/\\",
            )),
        ...operandPair(tag("or-true-skips"), disjunction, "\\/"),
        ...alternativePair(
          tag("selects"),
          replaced(unreachable, position, "x = 2"),
          replaced(poisoned, position, "7"),
          "label /= 7",
        ),
        ...alternativePair(
          tag("condition-error"),
          replaced(unreachable, position, poison.condition),
          plain,
          "label >= 0",
        ),
      ];
    }),
  );

const positionSources = (): readonly Source[] => {
  const count = controlLength;
  const guards = conjunctionTerms(count - 1);
  const actionTerms = ["(x' = IF x < 5 THEN x + 1 ELSE 0)", ...guards];
  const enabledTerms = ["x' = x + 1", ...guards];
  const binderTerms = [
    actionTerms[0] ?? "TRUE",
    ...indices(count - 1).map((index) =>
      index % 3 === 1
        ? `(\\E k${index} \\in {0} : x /= ${200 + index} + k${index})`
        : `x /= ${200 + index}`,
    ),
  ];
  const arms = {
    conditions: indices(54).map((index) => `x = ${index}`),
    values: indices(54).map((index) => `x' = ${index + 1}`),
  };
  const disjuncts = indices(count).map(
    (index) => `(x = ${index} /\\ x' = ${index + 1})`,
  );
  const specificationTerms = [
    ...indices(45).map((index) =>
      index === 0 ? "x = 0" : `x /= ${200 + index}`,
    ),
    "[][Next]_x",
  ];
  const sets = {
    conditions: indices(count).map((index) => `x = ${index}`),
    values: indices(count).map((index) => `{${index}}`),
  };
  const dispersed = indices(count).map((index) =>
    index % 3 === 2
      ? `(x /= ${200 + index} /\\ x /= ${500 + index})`
      : `x /= ${200 + index}`,
  );
  const action = (
    name: string,
    operand: string,
    equivalent: string | null,
  ): Source =>
    entry(
      name,
      "position",
      `Init == x = 0\nNext == ${operand}\nInv == x >= 0\n`,
      invariantConfig,
      equivalent,
    );
  const enabled = (
    name: string,
    operand: string,
    equivalent: string | null,
  ): Source =>
    entry(
      name,
      "position",
      `${counter(5)}Inv == ENABLED (${operand})\n`,
      invariantConfig,
      equivalent,
    );
  const specified = (
    name: string,
    operand: string,
    equivalent: string | null,
  ): Source =>
    entry(
      name,
      "position",
      `Next == x' = IF x < 5 THEN x + 1 ELSE 0\nSpec == ${operand}\nInv == x >= 0\n`,
      specificationConfig,
      equivalent,
    );
  const membership = (
    name: string,
    operand: string,
    equivalent: string | null,
  ): Source =>
    entry(
      name,
      "position",
      `${counter(5)}Types == ${operand}\nInv == x \\in Types\n`,
      invariantConfig,
      equivalent,
    );
  return [
    action(
      "next-and-paren-44",
      parenthesized("/\\", actionTerms),
      "next-and-flat-44",
    ),
    action("next-and-flat-44", flat("/\\", actionTerms), null),
    action(
      "next-and-bulleted-44",
      bulleted("/\\", actionTerms),
      "next-and-flat-44",
    ),
    action(
      "next-and-right-44",
      rightNested("/\\", actionTerms),
      "next-and-flat-44",
    ),
    action(
      "next-binders-paren-44",
      parenthesized("/\\", binderTerms),
      "next-binders-flat-44",
    ),
    action("next-binders-flat-44", flat("/\\", binderTerms), null),
    action(
      "next-binders-bulleted-44",
      bulleted("/\\", binderTerms),
      "next-binders-flat-44",
    ),
    action(
      "next-binders-right-44",
      rightNested("/\\", binderTerms),
      "next-binders-flat-44",
    ),
    action(
      "next-or-paren-44",
      parenthesized("\\/", disjuncts),
      "next-or-flat-44",
    ),
    action("next-or-flat-44", flat("\\/", disjuncts), null),
    action(
      "next-or-right-44",
      rightNested("\\/", disjuncts),
      "next-or-flat-44",
    ),
    action(
      "next-if-54",
      elseIf(arms.conditions, arms.values, "x' = 0"),
      "next-case-54",
    ),
    action(
      "next-case-54",
      caseOther(arms.conditions, arms.values, "x' = 0"),
      null,
    ),
    enabled(
      "enabled-and-paren-44",
      parenthesized("/\\", enabledTerms),
      "enabled-and-flat-44",
    ),
    enabled("enabled-and-flat-44", flat("/\\", enabledTerms), null),
    enabled(
      "enabled-and-right-44",
      rightNested("/\\", enabledTerms),
      "enabled-and-flat-44",
    ),
    specified(
      "specification-paren-45",
      parenthesized("/\\", specificationTerms),
      "specification-flat-45",
    ),
    specified("specification-flat-45", flat("/\\", specificationTerms), null),
    specified(
      "specification-right-45",
      rightNested("/\\", specificationTerms),
      "specification-flat-45",
    ),
    membership(
      "membership-if-44",
      elseIf(sets.conditions, sets.values, "{0}"),
      "membership-case-44",
    ),
    membership(
      "membership-case-44",
      caseOther(sets.conditions, sets.values, "{0}"),
      null,
    ),
    entry(
      "mixed-and-44",
      "position",
      `${counter(5)}Inv == ${parenthesized("/\\", dispersed)}\n`,
      invariantConfig,
      null,
      "flat-and-44",
    ),
  ];
};

const orderSources = (): readonly Source[] => {
  const unknown = (operands: readonly string[]) =>
    operands.map((operand, index) =>
      index === 4 ? "Early" : index === 30 ? "Late" : operand,
    );
  const label = (count: number, early: number) => {
    const { conditions, values } = stepArms(count);
    conditions[early + 1] = "Late = 0";
    values[early] = "Early";
    return `${counter(5)}label == ${elseIf(conditions, values, "0")}\nInv == label >= 0\n`;
  };
  return [
    ...pair(
      "order-and",
      "paren",
      "flat",
      `${counter(5)}Inv == ${parenthesized("/\\", unknown(conjunctionTerms(controlLength)))}\n`,
      `${counter(5)}Inv == ${flat("/\\", unknown(conjunctionTerms(controlLength)))}\n`,
    ).map((source) => ({ ...source, group: "order" })),
    ...pair(
      "order-or",
      "paren",
      "flat",
      `${counter(5)}Inv == ${parenthesized("\\/", unknown(disjunctionTerms(controlLength)))}\n`,
      `${counter(5)}Inv == ${flat("\\/", unknown(disjunctionTerms(controlLength)))}\n`,
    ).map((source) => ({ ...source, group: "order" })),
    entry("order-if-30", "order", label(30, 3), invariantConfig, null),
    entry(
      "order-if-44",
      "order",
      label(controlLength, 41),
      invariantConfig,
      null,
      "order-if-30",
    ),
  ];
};

const boundarySources = (): readonly Source[] => {
  const entries: Source[] = [];
  const add = (name: string, body: string, config = invariantConfig) =>
    entries.push(entry(name, "boundary", `${counter(5)}${body}`, config, null));
  for (const depth of boundaryDepths) {
    add(`negation-${depth}`, `Inv == ${"~".repeat(depth)}(x < 0)\n`);
    add(
      `identity-${depth}`,
      `Id(p) == p\nInv == ${"Id(".repeat(depth)}x >= 0${")".repeat(depth)}\n`,
    );
  }
  for (const length of [38, controlLength])
    add(
      `alternating-${length}`,
      `Inv == ${indices(length).reduce(
        (left, index) =>
          `(${left} ${index % 2 === 0 ? "/\\" : "\\/"} x /= ${200 + index})`,
        "x >= 0",
      )}\n`,
    );
  for (const length of [30, 39, 40, controlLength])
    add(
      `right-and-${length}`,
      `Inv == ${rightNested("/\\", conjunctionTerms(length))}\n`,
    );
  for (const length of [38, 39, controlLength])
    add(
      `union-${length}`,
      `Inv == x \\in ${parenthesized(
        "\\cup",
        indices(length).map((index) => `{${index}}`),
      )}\n`,
    );
  for (const length of [38, 39, controlLength])
    add(
      `if-then-${length}`,
      `label == ${indices(length)
        .map((index) => `IF x = ${index} THEN `)
        .join("")}0${indices(length)
        .map((index) => ` ELSE ${index}`)
        .join("")}\nInv == label >= 0\n`,
    );
  for (const length of [38, controlLength])
    add(
      `temporal-and-${length}`,
      `Live == ${parenthesized(
        "/\\",
        indices(length).map(() => "[]<>(x >= 0)"),
      )}\n`,
      propertyConfig,
    );
  for (const steps of [10, 22]) {
    add(
      `let-between-and-${steps}`,
      `Inv == ${indices(steps).reduce(
        (inner, index) => `((LET t == 0 IN ${inner}) /\\ x /= ${200 + index})`,
        "x >= 0",
      )}\n`,
    );
    add(
      `call-between-and-${steps}`,
      `Id(p) == p\nInv == ${indices(steps).reduce(
        (inner, index) => `Id(${inner} /\\ x /= ${200 + index})`,
        "x >= 0",
      )}\n`,
    );
  }
  return entries;
};

const limitSources = (): readonly Source[] => {
  const disjunction = (count: number) =>
    indices(count)
      .map(() => "(x' = 0)")
      .join(" \\/ ");
  const guard = "(x = 0 \\/ x = 1)";
  const copies = (count: number) => indices(count).map(() => guard);
  const left = (count: number) => parenthesized("/\\", copies(count));
  const right = (count: number, last: string) =>
    rightNested("/\\", [...copies(count), last]);
  const next = (name: string, body: string) =>
    entry(
      name,
      "limit",
      `Init == x = 0\nNext == ${body}\nInv == x >= 0\n`,
      invariantConfig,
      null,
    );
  const enabled = (name: string, body: string) =>
    entry(
      name,
      "limit",
      `${counter(5)}Inv == ENABLED (${body})\n`,
      invariantConfig,
      null,
    );
  const specified = (name: string, body: string) =>
    entry(
      name,
      "limit",
      `Init == x = 0\nSpec == Init /\\ [][${body}]_x\nInv == x >= 0\n`,
      specificationConfig,
      null,
    );
  const placements: readonly (readonly [string, string, boolean])[] = [
    ["or-inner-then-missing", `(${disjunction(4097)}) \\/ Missing`, true],
    ["or-missing-then-inner", `Missing \\/ (${disjunction(4097)})`, true],
    ["or-flat-then-missing", `${disjunction(4097)} \\/ Missing`, false],
    [
      "or-within-limit-then-missing",
      `(${disjunction(4096)}) \\/ Missing`,
      false,
    ],
    [
      "or-nested-left-then-missing",
      `((${disjunction(4097)}) \\/ x' = 1) \\/ Missing`,
      false,
    ],
    [
      "or-nested-right-then-missing",
      `(x' = 1 \\/ (${disjunction(4097)})) \\/ Missing`,
      false,
    ],
    ["and-chain-then-missing", `${left(13)} /\\ Missing`, true],
    ["and-missing-then-chain", `Missing /\\ ${left(13)}`, false],
    ["and-flat-then-missing", `${flat("/\\", copies(13))} /\\ Missing`, false],
    ["and-right-then-missing", right(13, "Missing"), true],
    ["and-within-limit-then-missing", `${left(12)} /\\ Missing`, false],
    [
      "and-chain-in-or-then-missing",
      `(${left(13)} /\\ Missing) \\/ x' = 0`,
      false,
    ],
    [
      "or-in-and-then-missing",
      `(${disjunction(4097)} \\/ Missing) /\\ x' = 1`,
      false,
    ],
    [
      "else-if-then-missing",
      `(IF x = 0 THEN (${disjunction(2049)}) ELSE IF x = 1 THEN (${disjunction(2049)}) ELSE (${disjunction(2049)})) /\\ Missing`,
      false,
    ],
  ];
  return [
    next("limit-next-or-flat-4096", disjunction(4096)),
    next("limit-next-or-flat-4097", disjunction(4097)),
    next("limit-next-and-chain-13", left(13)),
    ...placements.flatMap(([name, body, everywhere]) => [
      next(`limit-next-${name}`, body),
      ...(everywhere
        ? [
            enabled(`limit-enabled-${name}`, body),
            specified(`limit-specification-${name}`, body),
          ]
        : []),
    ]),
  ];
};

export const sources = (): readonly Source[] => [
  ...lengthSources(),
  ...semanticSources(),
  ...positionSources(),
  ...orderSources(),
  ...boundarySources(),
  ...limitSources(),
];
