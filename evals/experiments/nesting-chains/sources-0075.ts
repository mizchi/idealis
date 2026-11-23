import {
  caseOther,
  elseIf,
  entry,
  flat,
  parenthesized,
  rightNested,
  type Source,
} from "./sources.js";

const indices = (count: number): readonly number[] =>
  Array.from({ length: count }, (_, index) => index);
const counter = (limit: number): string =>
  `Init == x = 0\nNext == x' = IF x < ${limit} THEN x + 1 ELSE 0\n`;
const invariantConfig =
  "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n";
const walk = counter(5);

export const flatLengths = [100, 127, 128, 129] as const;
export const caseLengths = [100, 126, 127, 128] as const;
export const splits = [
  [2, 38],
  [2, 47],
  [3, 38],
  [3, 40],
  [3, 41],
  [4, 30],
  [4, 31],
  [5, 24],
  [5, 25],
] as const;
export const semanticPositions = [1, 27, 28, 54] as const;

const conjunctionTerms = (count: number): readonly string[] =>
  indices(count).map((index) => `x /= ${200 + index}`);
const disjunctionTerms = (count: number): readonly string[] =>
  indices(count).map((index) =>
    index === count - 1 ? "x >= 0" : `x = ${200 + index}`,
  );
const arms = (count: number) => ({
  conditions: indices(count).map((index) => `x = ${20 + index}`),
  values: indices(count).map(() => "FALSE"),
});

const lengthSources = (): readonly Source[] => [
  ...flatLengths.flatMap((length) => [
    entry(
      `flat-and-${length}`,
      "length-0075",
      `${walk}Inv == ${flat("/\\", conjunctionTerms(length))}\n`,
      invariantConfig,
      null,
    ),
    entry(
      `flat-or-${length}`,
      "length-0075",
      `${walk}Inv == ${flat("\\/", disjunctionTerms(length))}\n`,
      invariantConfig,
      null,
    ),
  ]),
  ...caseLengths.map((length) => {
    const { conditions, values } = arms(length);
    return entry(
      `case-other-${length}`,
      "length-0075",
      `${walk}Inv == ${caseOther(conditions, values, "TRUE")}\n`,
      invariantConfig,
      null,
    );
  }),
];

const splitSources = (): readonly Source[] =>
  splits.flatMap(([count, length]) => {
    const { conditions, values } = arms(length);
    const next = (index: number) =>
      index + 1 === count ? "TRUE" : `D${index + 1}(k)`;
    const chain = (index: number, form: "if" | "and") =>
      form === "if"
        ? elseIf(conditions, values, next(index))
        : parenthesized("/\\", [...conjunctionTerms(length), next(index)]);
    return (["if", "and"] as const).map((form) =>
      entry(
        `split-${form}-${count}x${length}`,
        "split",
        `${walk}${[...indices(count)]
          .reverse()
          .map((index) => `D${index}(k) == ${chain(index, form)}\n`)
          .join("")}Inv == D0(1)\n`,
        invariantConfig,
        null,
      ),
    );
  });

const replaced = (
  operands: readonly string[],
  position: number,
  replacement: string,
): readonly string[] =>
  operands.map((operand, index) =>
    index === position - 1 ? replacement : operand,
  );

const division = "10 \\div (3 - x) /= 0";
const unreachable = "(1 \\div 0) = 0";

const semanticSources = (): readonly Source[] =>
  semanticPositions.flatMap((position): readonly Source[] => {
    const conjunction = (operands: readonly string[]) =>
      `${walk}Inv == ${flat("/\\", operands)}\n`;
    const falseAt = replaced(conjunctionTerms(60), position, "x /= 3");
    const skips = replaced(falseAt, position + 1, division);
    const errorFirst = replaced(
      replaced(conjunctionTerms(60), position, division),
      position + 1,
      "x /= 3",
    );
    const nonBoolean = replaced(conjunctionTerms(60), position, "1");
    const trueAt = replaced(
      replaced(
        indices(60).map((index) => `x = ${300 + index}`),
        position,
        "x >= 0",
      ),
      position + 1,
      unreachable,
    );
    const links = (condition: string, value: string) => ({
      conditions: replaced(
        indices(54).map((index) => `x = ${1000 + index}`),
        position,
        condition,
      ),
      values: replaced(
        indices(54).map(() => unreachable),
        position,
        value,
      ),
    });
    const chain = (condition: string, value: string) => {
      const { conditions, values } = links(condition, value);
      return `${walk}Inv == ${elseIf(conditions, values, unreachable)}\n`;
    };
    const tag = (kind: string) => `${kind}-${position}`;
    return [
      entry(
        tag("and-false"),
        "semantics-0075",
        conjunction(falseAt),
        invariantConfig,
        null,
      ),
      entry(
        tag("and-false-skips"),
        "semantics-0075",
        conjunction(skips),
        invariantConfig,
        null,
      ),
      entry(
        tag("and-error-first"),
        "semantics-0075",
        conjunction(errorFirst),
        invariantConfig,
        null,
      ),
      entry(
        tag("and-non-boolean"),
        "semantics-0075",
        conjunction(nonBoolean),
        invariantConfig,
        null,
      ),
      entry(
        tag("or-true-skips"),
        "semantics-0075",
        `${walk}Inv == ${flat("\\/", trueAt)}\n`,
        invariantConfig,
        null,
      ),
      entry(
        tag("if-selects"),
        "semantics-0075",
        chain("x >= 0", "x /= 3"),
        invariantConfig,
        null,
      ),
      entry(
        tag("if-condition-non-boolean"),
        "semantics-0075",
        chain("5", "TRUE"),
        invariantConfig,
        null,
      ),
      entry(
        tag("if-condition-error"),
        "semantics-0075",
        chain(division, "TRUE"),
        invariantConfig,
        null,
      ),
    ];
  });

const positionSources = (): readonly Source[] => {
  const { conditions, values } = arms(54);
  const sets = indices(54).map((index) => `{${100 + index}}`);
  const keyed = indices(54).map((index) => `k = ${20 + index}`);
  const guards = indices(54).map((index) => `x = ${1000 + index}`);
  const recursive = (links: number, start: number) =>
    `${walk}RECURSIVE R(_)\nR(n) == ${elseIf(
      guards.slice(0, links),
      values.slice(0, links),
      "IF n = 0 THEN x >= 0 ELSE R(n - 1)",
    )}\nInv == R(${start})\n`;
  return [
    entry(
      "membership-if-54",
      "position-0075",
      `${walk}Types == ${elseIf(conditions, sets, "{0, 1, 2, 3, 4, 5}")}\nInv == x \\in Types\n`,
      invariantConfig,
      null,
    ),
    entry(
      "lazy-body-if-54",
      "position-0075",
      `${walk}F == [k \\in 0..5 |-> ${elseIf(keyed, values, "TRUE")}]\nInv == F[x]\n`,
      invariantConfig,
      null,
    ),
    ...[3, 30, 40, 60].flatMap((start) => [
      entry(
        `recursive-if-54-from-${start}`,
        "position-0075",
        recursive(54, start),
        invariantConfig,
        null,
        `recursive-plain-from-${start}`,
      ),
      entry(
        `recursive-plain-from-${start}`,
        "position-0075",
        recursive(0, start),
        invariantConfig,
        null,
      ),
    ]),
  ];
};

const boundarySources = (): readonly Source[] => [
  ...[47, 48, 49].map((length) =>
    entry(
      `implies-${length}`,
      "boundary-0075",
      `${walk}Inv == ${parenthesized("=>", conjunctionTerms(length))}\n`,
      invariantConfig,
      null,
    ),
  ),
  ...[47, 48, 49].map((length) =>
    entry(
      `right-or-${length}`,
      "boundary-0075",
      `${walk}Inv == ${rightNested("\\/", [...disjunctionTerms(length)].reverse())}\n`,
      invariantConfig,
      null,
    ),
  ),
  ...[127, 128, 129].map((count) =>
    entry(
      `calls-${count}`,
      "boundary-0075",
      `${walk}${[...indices(count)]
        .reverse()
        .map(
          (index) =>
            `C${index} == ${index + 1 === count ? "x >= 0" : `C${index + 1}`}\n`,
        )
        .join("")}Inv == C0\n`,
      invariantConfig,
      null,
    ),
  ),
];

export const sources0075 = (): readonly Source[] => [
  ...lengthSources(),
  ...splitSources(),
  ...semanticSources(),
  ...positionSources(),
  ...boundarySources(),
];
