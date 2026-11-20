import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import type {
  PortableCommand,
  SafetyProgramV2,
} from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyProgram } from "../../packages/idealis/src/safety-program.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

const truth = { op: "literal", value: { type: "bool", value: true } } as const;
const zero = { op: "literal", value: { type: "int", value: "0" } } as const;
const nine = { op: "literal", value: { type: "int", value: "9" } } as const;
const four = { op: "literal", value: { type: "int", value: "4" } } as const;
const parameter = { op: "parameter", slot: 0 } as const;
const invalidParameter = { op: "parameter", slot: 2 } as const;
const frame = (body: unknown, actual: unknown = truth, localScope = 2) => ({
  op: "frame",
  arguments: [actual],
  captures: [],
  parameterCount: 1,
  captureCount: 0,
  localScope,
  body,
});
const binding = { slot: 0, domain: { kind: "set", values: [zero] } };
const branch = {
  name: "Body",
  steps: [{ kind: "assign", slot: 0, value: zero }],
};
const signature = {
  arguments: [truth, truth],
  captures: [truth, truth],
  parameterCount: 2,
  captureCount: 2,
  localScope: 2,
};
const base = {
  format: "modelt.safety-program",
  version: 2,
  name: "AnalysisIR",
  variables: ["x"],
  initial: [
    { name: "Init", steps: [{ kind: "assign", slot: 0, value: zero }] },
  ],
  next: [
    {
      name: "Next",
      steps: [
        {
          kind: "assign",
          slot: 0,
          value: { op: "read", slot: 0, next: false },
        },
      ],
    },
  ],
  invariants: [],
  assumptions: [],
  checkDeadlock: false,
  definitions: [
    {
      id: 0,
      parameterCount: 2,
      captureCount: 2,
      localScope: 2,
      bodyKind: "expression",
      body: truth,
    },
  ],
};
const bounds = { maxDepth: 8, maxStates: 100, maxWork: 10_000 };
const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const native = (program: SafetyProgramV2): unknown => {
  const request: PortableCommand = {
    version: 1,
    request: { kind: "check-safety", program, bounds },
  };
  const result = spawnSync(
    "crates/idealis-core/target/release/idealis-core",
    [],
    {
      input: `${JSON.stringify(request)}\n`,
      encoding: "utf8",
      timeout: 1000,
      maxBuffer: 4 * 1024 * 1024,
    },
  );
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
};
const wasm = (program: SafetyProgramV2): unknown => {
  try {
    return core.request({
      version: 1,
      request: { kind: "check-safety", program, bounds },
    });
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error", code: error.code, message: error.message };
  }
};
const response = (program: unknown): unknown => {
  const decoded = decodeSafetyProgram(program);
  assert.equal(decoded.version, 2);
  const result = wasm(decoded as SafetyProgramV2);
  assert.deepEqual(native(decoded as SafetyProgramV2), result);
  return result;
};
const complete = (result: unknown) => {
  assert.ok(isRecord(result));
  assert.equal(result.kind, "safety", JSON.stringify(result));
  assert.ok(isRecord(result.result));
  assert.equal(result.result.status, "complete");
  assert.equal(result.result.reason, null);
  assert.equal(result.result.states, 1);
  assert.equal(result.result.generated, 2);
  assert.equal(result.result.depth, 0);
  assert.equal(result.result.violation, null);
};
const refused = (result: unknown, message: string) => {
  assert.deepEqual(result, { kind: "error", code: "protocol", message });
};
type Path = readonly (string | number)[];
const replaced = (value: unknown, path: Path, child: unknown): unknown => {
  if (path.length === 0) return child;
  const [key, ...rest] = path;
  assert.ok(key !== undefined);
  if (Array.isArray(value)) {
    assert.equal(typeof key, "number");
    return value.map((entry, index) =>
      index === key ? replaced(entry, rest, child) : entry,
    );
  }
  assert.ok(isRecord(value));
  return { ...value, [key]: replaced(value[key], rest, child) };
};
interface Edge {
  readonly id: string;
  readonly space: string;
  readonly constructor: string;
  readonly position: string;
  readonly value: unknown;
  readonly path: Path;
  readonly action?: boolean;
}
const edges: Edge[] = [];
const edge = (
  nodeKind: string,
  value: unknown,
  paths: readonly Path[],
  action = false,
) => {
  for (const path of paths)
    edges.push({
      id: `expression/${nodeKind}/${path.join("/")}`,
      space: "ir-expression",
      constructor: nodeKind,
      position: path
        .map((key, index) =>
          typeof key === "number"
            ? `[${key}]`
            : `${index > 0 ? "." : ""}${key}`,
        )
        .join(""),
      value,
      path,
      ...(action ? { action } : {}),
    });
};
for (const op of ["set", "tuple"])
  edge(op, { op, values: [truth, truth] }, [
    ["values", 0],
    ["values", 1],
  ]);
for (const op of ["record", "record-set"])
  edge(
    op,
    {
      op,
      fields: [
        ["a", truth],
        ["b", truth],
      ],
    },
    [
      ["fields", 0, 1],
      ["fields", 1, 1],
    ],
  );
edge("at", { op: "at", target: { op: "tuple", values: [truth] }, key: zero }, [
  ["target"],
  ["key"],
]);
edge(
  "update",
  {
    op: "update",
    target: { op: "tuple", values: [truth] },
    updates: [
      { path: [zero, zero], value: truth },
      { path: [zero, zero], value: truth },
    ],
  },
  [
    ["target"],
    ["updates", 0, "path", 0],
    ["updates", 0, "path", 1],
    ["updates", 1, "path", 0],
    ["updates", 1, "path", 1],
    ["updates", 0, "value"],
    ["updates", 1, "value"],
  ],
);
for (const op of ["forall", "exists", "function", "filter", "map-set"])
  edge(op, { op, bounds: [binding, { ...binding, slot: 1 }], body: truth }, [
    ["bounds", 0, "domain", "values", 0],
    ["bounds", 1, "domain", "values", 0],
    ["body"],
  ]);
edge("choose", { op: "choose", binding, body: truth }, [
  ["binding", "domain", "values", 0],
  ["body"],
]);
edge(
  "fold",
  {
    op: "fold",
    accumulator: 0,
    initial: zero,
    collection: { op: "tuple", values: [zero] },
    order: "sequence-left",
    body: zero,
  },
  [["initial"], ["collection"], ["body"]],
);
for (const op of [
  "sequence-set",
  "domain",
  "variant-tag",
  "cardinality",
  "len",
  "head",
  "tail",
  "powerset",
  "union",
  "not",
  "neg",
  "prime",
])
  edge(op, { op, argument: truth }, [["argument"]], op === "prime");
for (const op of [
  "cup",
  "cap",
  "difference",
  "subset-eq",
  "append",
  "concat",
  "function-set",
  "function-in",
  "sequence-in",
  "add",
  "sub",
  "mul",
  "div",
  "mod",
  "pow",
  "eq",
  "ne",
  "lt",
  "le",
  "gt",
  "ge",
  "and",
  "or",
  "implies",
])
  edge(op, { op, left: truth, right: truth }, [["left"], ["right"]]);
edge(
  "subseq",
  {
    op: "subseq",
    sequence: { op: "tuple", values: [truth] },
    from: zero,
    to: zero,
  },
  [["sequence"], ["from"], ["to"]],
);
edge(
  "enabled",
  { op: "enabled", branches: [branch, branch], subscript: zero },
  [
    ["branches", 0, "steps", 0, "value"],
    ["branches", 1, "steps", 0, "value"],
    ["subscript"],
  ],
);
edge(
  "if",
  { op: "if", condition: truth, consequent: truth, alternative: truth },
  [["condition"], ["consequent"], ["alternative"]],
);
edge("in", { op: "in", value: zero, domain: { kind: "set", values: [zero] } }, [
  ["value"],
  ["domain", "values", 0],
]);
edge("frame", { op: "frame", ...signature, body: truth }, [
  ["arguments", 0],
  ["arguments", 1],
  ["captures", 0],
  ["captures", 1],
  ["body"],
]);
edge(
  "call",
  {
    op: "call",
    definition: 0,
    arguments: [truth, truth],
    captures: [truth, truth],
  },
  [
    ["arguments", 0],
    ["arguments", 1],
    ["captures", 0],
    ["captures", 1],
  ],
);
const unreachable = (child: unknown) => ({
  op: "if",
  condition: truth,
  consequent: truth,
  alternative: child,
});
const atExpression = (entry: Edge, child: unknown) => {
  const body = frame(unreachable(replaced(entry.value, entry.path, child)));
  return {
    ...base,
    ...(entry.action
      ? { actionProperties: [{ name: "Safe", expression: body }] }
      : { invariants: [{ name: "Safe", expression: body }] }),
  };
};
const analysisIrManifest = edges.flatMap((entry) =>
  ["valid", "invalid"].map((control) => ({
    id: `ir/${entry.id}/${control}`,
    routes: 3,
    space: entry.space,
    constructor: entry.constructor,
    position: entry.position,
    control,
  })),
);
for (const entry of edges) {
  test(`analysis parity IR structural ${entry.id} valid`, () =>
    complete(response(atExpression(entry, parameter))));
  test(`analysis parity IR structural ${entry.id} invalid`, () =>
    refused(
      response(atExpression(entry, invalidParameter)),
      "Invalid safety parameter reference",
    ));
}
const eq = (left: unknown, right: unknown) => ({ op: "eq", left, right });
const semanticCases: readonly {
  readonly id: string;
  readonly program: unknown;
  readonly error?: string;
}[] = [
  {
    id: "expression-frame-caller-binder",
    program: {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: {
            op: "forall",
            bounds: [{ slot: 0, domain: { kind: "set", values: [nine] } }],
            body: frame(
              {
                op: "forall",
                bounds: [{ slot: 0, domain: { kind: "set", values: [four] } }],
                body: eq(parameter, nine),
              },
              { op: "bound", slot: 0 },
            ),
          },
        },
      ],
    },
  },
  {
    id: "expression-frame-caller-old",
    program: {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: eq(
            {
              op: "at",
              target: {
                op: "update",
                target: { op: "tuple", values: [nine] },
                updates: [
                  {
                    path: [
                      { op: "literal", value: { type: "int", value: "1" } },
                    ],
                    value: frame(
                      {
                        op: "at",
                        target: {
                          op: "update",
                          target: { op: "tuple", values: [four] },
                          updates: [
                            {
                              path: [
                                {
                                  op: "literal",
                                  value: { type: "int", value: "1" },
                                },
                              ],
                              value: parameter,
                            },
                          ],
                        },
                        key: {
                          op: "literal",
                          value: { type: "int", value: "1" },
                        },
                      },
                      { op: "old" },
                    ),
                  },
                ],
              },
              key: { op: "literal", value: { type: "int", value: "1" } },
            },
            nine,
          ),
        },
      ],
    },
  },
  {
    id: "expression-frame-capture",
    program: {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: {
            op: "frame",
            arguments: [],
            captures: [truth],
            parameterCount: 0,
            captureCount: 1,
            localScope: 0,
            body: { op: "capture", slot: 0 },
          },
        },
      ],
    },
  },
  {
    id: "expression-frame-unbound-capture",
    program: {
      ...base,
      invariants: [
        { name: "Safe", expression: frame({ op: "capture", slot: 0 }) },
      ],
    },
    error: "Invalid safety capture reference",
  },
  {
    id: "frame-unused-actual-structurally-valid-lazy",
    program: {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: frame(truth, {
            op: "head",
            argument: { op: "tuple", values: [] },
          }),
        },
      ],
    },
  },
  {
    id: "frame-used-actual-error",
    program: {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: frame(parameter, {
            op: "head",
            argument: { op: "tuple", values: [] },
          }),
        },
      ],
    },
    error: "Head of an empty sequence",
  },
  {
    id: "call-unused-actual-unknown-reference",
    program: {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: {
            op: "call",
            definition: 0,
            arguments: [
              truth,
              { op: "call", definition: 1, arguments: [], captures: [] },
            ],
            captures: [truth, truth],
          },
        },
      ],
    },
    error: "Unknown safety definition ID",
  },
  {
    id: "next-read-in-state",
    program: {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: eq({ op: "read", slot: 0, next: true }, zero),
        },
      ],
    },
    error: "Invalid safety variable read",
  },
  {
    id: "prime-in-state",
    program: {
      ...base,
      invariants: [
        { name: "Safe", expression: { op: "prime", argument: truth } },
      ],
    },
    error: "Prime requires an action context",
  },
  {
    id: "action-prime-read",
    program: {
      ...base,
      actionProperties: [
        {
          name: "Safe",
          expression: eq(
            { op: "prime", argument: { op: "read", slot: 0, next: false } },
            { op: "read", slot: 0, next: true },
          ),
        },
      ],
    },
  },
  {
    id: "action-frame-caller-actual",
    program: {
      ...base,
      next: [
        {
          name: "Next",
          steps: [
            {
              kind: "frame",
              arguments: [{ op: "read", slot: 0, next: false }],
              captures: [],
              parameterCount: 1,
              captureCount: 0,
              localScope: 0,
              branches: [
                {
                  name: "Body",
                  steps: [{ kind: "assign", slot: 0, value: parameter }],
                },
              ],
            },
          ],
        },
      ],
    },
  },
  {
    id: "action-frame-caller-capture",
    program: {
      ...base,
      next: [
        {
          name: "Next",
          steps: [
            {
              kind: "frame",
              arguments: [],
              captures: [{ op: "read", slot: 0, next: false }],
              parameterCount: 0,
              captureCount: 1,
              localScope: 0,
              branches: [
                {
                  name: "Body",
                  steps: [
                    {
                      kind: "assign",
                      slot: 0,
                      value: { op: "capture", slot: 0 },
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    },
  },
  {
    id: "temporal-frame-formula-actual",
    program: {
      ...base,
      properties: [
        {
          name: "Live",
          formula: {
            kind: "frame",
            arguments: [eq({ op: "read", slot: 0, next: false }, zero)],
            captures: [],
            parameterCount: 1,
            captureCount: 0,
            localScope: 0,
            formula: {
              kind: "always",
              formula: { kind: "atom", expression: parameter },
            },
          },
        },
      ],
    },
  },
  {
    id: "temporal-frame-formula-capture",
    program: {
      ...base,
      properties: [
        {
          name: "Live",
          formula: {
            kind: "frame",
            arguments: [],
            captures: [eq({ op: "read", slot: 0, next: false }, zero)],
            parameterCount: 0,
            captureCount: 1,
            localScope: 0,
            formula: { kind: "atom", expression: { op: "capture", slot: 0 } },
          },
        },
      ],
    },
  },
];
const analysisIrSemanticManifest = semanticCases.map((entry) => ({
  id: `ir/semantic/${entry.id}`,
  routes: 3,
  classification: entry.error ? "structural-or-evaluation-refusal" : "semantic",
}));
for (const entry of semanticCases)
  test(`analysis parity IR semantic ${entry.id}`, () => {
    const result = response(entry.program);
    if (entry.error) refused(result, entry.error);
    else complete(result);
  });
test("analysis parity IR manifest has unique IDs and stays within reserved complement", () => {
  const entries = [...analysisIrManifest, ...analysisIrSemanticManifest];
  assert.equal(new Set(entries.map((entry) => entry.id)).size, entries.length);
  assert.ok(entries.length <= 768);
  assert.ok(entries.every((entry) => entry.routes === 3));
  assert.deepEqual(
    new Set(edges.map((entry) => entry.constructor)),
    new Set([
      "set",
      "tuple",
      "record",
      "record-set",
      "at",
      "update",
      "forall",
      "exists",
      "function",
      "filter",
      "map-set",
      "choose",
      "fold",
      "sequence-set",
      "domain",
      "variant-tag",
      "cardinality",
      "len",
      "head",
      "tail",
      "powerset",
      "union",
      "not",
      "neg",
      "prime",
      "cup",
      "cap",
      "difference",
      "subset-eq",
      "append",
      "concat",
      "function-set",
      "function-in",
      "sequence-in",
      "add",
      "sub",
      "mul",
      "div",
      "mod",
      "pow",
      "eq",
      "ne",
      "lt",
      "le",
      "gt",
      "ge",
      "and",
      "or",
      "implies",
      "subseq",
      "enabled",
      "if",
      "in",
      "frame",
      "call",
    ]),
  );
});

interface WrapperEdge {
  readonly id: string;
  readonly program: unknown;
  readonly path: Path;
}
const wrapperEdges: WrapperEdge[] = [];
const addWrapper = (id: string, program: unknown, path: Path) =>
  wrapperEdges.push({ id, program, path });
for (const [domain, paths] of [
  [
    { kind: "value", value: { op: "set", values: [parameter] } },
    [["value", "values", 0]],
  ],
  [{ kind: "range", lower: zero, upper: zero }, [["lower"], ["upper"]]],
  [
    { kind: "set", values: [parameter, parameter] },
    [
      ["values", 0],
      ["values", 1],
    ],
  ],
] as const)
  for (const path of paths) {
    const body = frame(
      unreachable({ op: "exists", bounds: [{ slot: 0, domain }], body: truth }),
    );
    const program = {
      ...base,
      invariants: [{ name: "Safe", expression: body }],
    };
    addWrapper(`domain/${domain.kind}/${path.join("/")}`, program, [
      "invariants",
      0,
      "expression",
      "body",
      "alternative",
      "bounds",
      0,
      "domain",
      ...path,
    ]);
  }
const falsehood = {
  op: "literal",
  value: { type: "bool", value: false },
} as const;
const actionSteps = [
  { value: { kind: "guard", expression: truth }, paths: [["expression"]] },
  { value: { kind: "assign", slot: 0, value: zero }, paths: [["value"]] },
  {
    value: {
      kind: "bind",
      slot: 0,
      domain: { kind: "set", values: [zero, zero] },
    },
    paths: [
      ["domain", "values", 0],
      ["domain", "values", 1],
    ],
  },
  {
    value: {
      kind: "choose",
      slot: 0,
      domain: { kind: "set", values: [zero, zero] },
    },
    paths: [
      ["domain", "values", 0],
      ["domain", "values", 1],
    ],
  },
  {
    value: {
      kind: "frame",
      ...signature,
      branches: [
        { name: "a", steps: [{ kind: "guard", expression: truth }] },
        { name: "b", steps: [{ kind: "guard", expression: truth }] },
      ],
    },
    paths: [
      ["arguments", 0],
      ["arguments", 1],
      ["captures", 0],
      ["captures", 1],
      ["branches", 0, "steps", 0, "expression"],
      ["branches", 1, "steps", 0, "expression"],
    ],
  },
] as const;
for (const entry of actionSteps)
  for (const path of entry.paths) {
    const program = {
      ...base,
      next: [
        {
          name: "Unreached",
          steps: [
            {
              kind: "frame",
              arguments: [truth],
              captures: [],
              parameterCount: 1,
              captureCount: 0,
              localScope: 2,
              branches: [
                {
                  name: "Body",
                  steps: [
                    { kind: "guard", expression: falsehood },
                    entry.value,
                  ],
                },
              ],
            },
          ],
        },
        ...base.next,
      ],
    };
    addWrapper(`step/${entry.value.kind}/${path.join("/")}`, program, [
      "next",
      0,
      "steps",
      0,
      "branches",
      0,
      "steps",
      1,
      ...path,
    ]);
  }
const temporalFormulas = [
  { value: { kind: "atom", expression: truth }, paths: [["expression"]] },
  ...["not", "always", "eventually"].map((kind) => ({
    value: { kind, formula: { kind: "atom", expression: truth } },
    paths: [["formula", "expression"]],
  })),
  ...["and", "or"].map((kind) => ({
    value: {
      kind,
      left: { kind: "atom", expression: truth },
      right: { kind: "atom", expression: truth },
    },
    paths: [
      ["left", "expression"],
      ["right", "expression"],
    ],
  })),
  ...["forall", "exists"].map((kind) => ({
    value: {
      kind,
      bounds: [{ slot: 0, domain: { kind: "set", values: [truth] } }],
      formula: { kind: "atom", expression: truth },
    },
    paths: [
      ["bounds", 0, "domain", "values", 0],
      ["formula", "expression"],
    ],
  })),
  {
    value: {
      kind: "frame",
      ...signature,
      formula: { kind: "atom", expression: truth },
    },
    paths: [
      ["arguments", 0],
      ["arguments", 1],
      ["captures", 0],
      ["captures", 1],
      ["formula", "expression"],
    ],
  },
];
for (const entry of temporalFormulas)
  for (const path of entry.paths) {
    const formula =
      entry.value.kind === "not"
        ? { kind: "not", formula: entry.value }
        : entry.value;
    const program = {
      ...base,
      properties: [
        {
          name: "Live",
          formula: {
            kind: "frame",
            arguments: [truth],
            captures: [],
            parameterCount: 1,
            captureCount: 0,
            localScope: 2,
            formula,
          },
        },
      ],
    };
    addWrapper(`temporal/${entry.value.kind}/${path.join("/")}`, program, [
      "properties",
      0,
      "formula",
      "formula",
      ...(entry.value.kind === "not" ? ["formula"] : []),
      ...path,
    ]);
  }
for (const entry of wrapperEdges) {
  test(`analysis parity IR structural wrapper ${entry.id} valid`, () => {
    const value = entry.id.startsWith("domain/range/") ? zero : parameter;
    complete(response(replaced(entry.program, entry.path, value)));
  });
  test(`analysis parity IR structural wrapper ${entry.id} invalid`, () =>
    refused(
      response(replaced(entry.program, entry.path, invalidParameter)),
      "Invalid safety parameter reference",
    ));
}
test("analysis parity IR wrapper manifest covers domains steps and temporal frames", () => {
  assert.equal(
    new Set(wrapperEdges.map((entry) => entry.id)).size,
    wrapperEdges.length,
  );
  assert.ok(
    analysisIrManifest.length +
      analysisIrSemanticManifest.length +
      wrapperEdges.length * 2 <=
      768,
  );
  assert.equal(
    wrapperEdges.filter((entry) => entry.id.startsWith("domain/")).length,
    5,
  );
  assert.equal(
    wrapperEdges.filter((entry) => entry.id.startsWith("step/")).length,
    12,
  );
  assert.equal(
    wrapperEdges.filter((entry) => entry.id.startsWith("temporal/")).length,
    17,
  );
});

const rootEdges: WrapperEdge[] = [];
for (const field of [
  "invariants",
  "assumptions",
  "initialProperties",
  "actionProperties",
  "constraints",
  "actionConstraints",
] as const)
  rootEdges.push({
    id: `program/${field}`,
    program: {
      ...base,
      [field]: [{ name: "Root", expression: frame(parameter) }],
    },
    path: [field, 0, "expression", "body"],
  });
rootEdges.push({
  id: "program/definitions",
  program: {
    ...base,
    definitions: [{ ...base.definitions[0], body: parameter }],
  },
  path: ["definitions", 0, "body"],
});
rootEdges.push({
  id: "program/fairness",
  program: {
    ...base,
    fairness: [
      {
        kind: "frame",
        arguments: [truth],
        captures: [],
        parameterCount: 1,
        captureCount: 0,
        localScope: 0,
        formula: { kind: "atom", expression: parameter },
      },
    ],
  },
  path: ["fairness", 0, "formula", "expression"],
});
for (const entry of rootEdges) {
  test(`analysis parity IR structural wrapper ${entry.id} valid`, () =>
    complete(response(entry.program)));
  test(`analysis parity IR structural wrapper ${entry.id} invalid`, () =>
    refused(
      response(replaced(entry.program, entry.path, invalidParameter)),
      "Invalid safety parameter reference",
    ));
}
for (const [op, valid, invalid, message] of [
  [
    "bound",
    {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: {
            op: "forall",
            bounds: [binding],
            body: eq({ op: "bound", slot: 0 }, zero),
          },
        },
      ],
    },
    {
      ...base,
      invariants: [{ name: "Safe", expression: { op: "bound", slot: 0 } }],
    },
    "Unbound quantified variable",
  ],
  [
    "read",
    {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: eq({ op: "read", slot: 0, next: false }, zero),
        },
      ],
    },
    {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: eq({ op: "read", slot: 1, next: false }, zero),
        },
      ],
    },
    "Invalid safety variable read",
  ],
  [
    "old",
    {
      ...base,
      invariants: [
        {
          name: "Safe",
          expression: eq(
            {
              op: "at",
              target: {
                op: "update",
                target: { op: "tuple", values: [nine] },
                updates: [
                  {
                    path: [
                      { op: "literal", value: { type: "int", value: "1" } },
                    ],
                    value: { op: "old" },
                  },
                ],
              },
              key: { op: "literal", value: { type: "int", value: "1" } },
            },
            nine,
          ),
        },
      ],
    },
    { ...base, invariants: [{ name: "Safe", expression: { op: "old" } }] },
    "Unbound EXCEPT @",
  ],
] as const) {
  test(`analysis parity IR leaf ${op} valid scope`, () =>
    complete(response(valid)));
  test(`analysis parity IR leaf ${op} invalid scope`, () =>
    refused(response(invalid), message));
}
for (const [id, expression] of [
  ["literal", truth],
  ["data", { op: "data", value: { type: "bool", value: true } }],
] as const)
  test(`analysis parity IR leaf ${id} sentinel`, () =>
    complete(
      response({ ...base, invariants: [{ name: "Safe", expression }] }),
    ));
test("analysis parity IR definition graph refuses unreachable self cycles", () => {
  const body = {
    op: "call",
    definition: 0,
    arguments: [parameter, { op: "parameter", slot: 1 }],
    captures: [
      { op: "capture", slot: 0 },
      { op: "capture", slot: 1 },
    ],
  };
  refused(
    response({ ...base, definitions: [{ ...base.definitions[0], body }] }),
    "Recursive safety definition graph",
  );
});
for (const depth of [48, 49])
  test(`analysis parity IR definition expression depth ${depth}`, () => {
    let body: unknown = truth;
    for (let index = 0; index < depth; index++)
      body = { op: "not", argument: body };
    const result = response({
      ...base,
      definitions: [{ ...base.definitions[0], body }],
    });
    if (depth === 48) complete(result);
    else refused(result, "Safety expression nesting exceeds 48");
  });
test("analysis parity IR all supplemental counts stay below 768", () => {
  const total =
    analysisIrManifest.length +
    analysisIrSemanticManifest.length +
    wrapperEdges.length * 2 +
    rootEdges.length * 2 +
    11;
  assert.equal(total, 343);
  assert.ok(total <= 768);
});

const semanticPositions: Readonly<Record<string, readonly string[]>> = {
  "expression-frame-caller-binder": [
    "ir-expression/frame/arguments[*]",
    "ir-expression/frame/body",
    "ir-expression/forall/body",
    "ir-expression/bound",
    "ir-expression/parameter",
  ],
  "expression-frame-caller-old": [
    "ir-expression/update/updates[*].value",
    "ir-expression/frame/arguments[*]",
    "ir-expression/frame/body",
    "ir-expression/old",
  ],
  "expression-frame-capture": [
    "ir-expression/frame/captures[*]",
    "ir-expression/frame/body",
    "ir-expression/capture",
  ],
  "expression-frame-unbound-capture": [
    "ir-expression/frame/body",
    "ir-expression/capture",
  ],
  "frame-unused-actual-structurally-valid-lazy": [
    "ir-expression/frame/arguments[*]",
    "ir-expression/frame/body",
    "ir-expression/head/argument",
  ],
  "frame-used-actual-error": [
    "ir-expression/frame/arguments[*]",
    "ir-expression/frame/body",
    "ir-expression/head/argument",
  ],
  "call-unused-actual-unknown-reference": [
    "ir-expression/call/arguments[*]",
    "ir-expression/call/definition",
  ],
  "next-read-in-state": ["ir-expression/read"],
  "prime-in-state": ["ir-expression/prime/argument"],
  "action-prime-read": ["ir-expression/prime/argument", "ir-expression/read"],
  "action-frame-caller-actual": [
    "ir-step/frame/arguments[*]",
    "ir-step/frame/branches[*]",
    "ir-step/assign/value",
  ],
  "action-frame-caller-capture": [
    "ir-step/frame/captures[*]",
    "ir-step/frame/branches[*]",
    "ir-step/assign/value",
  ],
  "temporal-frame-formula-actual": [
    "ir-temporal/frame/arguments[*]",
    "ir-temporal/frame/formula",
    "ir-temporal/always/formula",
    "ir-temporal/atom/expression",
  ],
  "temporal-frame-formula-capture": [
    "ir-temporal/frame/captures[*]",
    "ir-temporal/frame/formula",
    "ir-temporal/atom/expression",
  ],
};
const runtimeIrManifest = [
  ...analysisIrManifest.map((entry) => ({
    ...entry,
    classification: "structural",
    expected:
      entry.control === "valid"
        ? "complete"
        : "Invalid safety parameter reference",
  })),
  ...semanticCases.map((entry) => ({
    id: `ir/semantic/${entry.id}`,
    routes: 3,
    classification: "semantic",
    expected: entry.error ?? "complete",
    positions: semanticPositions[entry.id],
  })),
  ...wrapperEdges.flatMap((entry) =>
    ["valid", "invalid"].map((control) => ({
      id: `ir/wrapper/${entry.id}/${control}`,
      routes: 3,
      classification: "structural",
      control,
      expected:
        control === "valid" ? "complete" : "Invalid safety parameter reference",
      positions: [entry.path.join("/")],
    })),
  ),
  ...rootEdges.flatMap((entry) =>
    ["valid", "invalid"].map((control) => ({
      id: `ir/wrapper/${entry.id}/${control}`,
      routes: 3,
      classification: "structural",
      control,
      expected:
        control === "valid" ? "complete" : "Invalid safety parameter reference",
      positions: [entry.path.join("/")],
    })),
  ),
  ...["bound", "read", "old"].flatMap((op) =>
    ["valid", "invalid"].map((control) => ({
      id: `ir/leaf/${op}/${control}`,
      routes: 3,
      classification: "scope",
      control,
      expected:
        control === "valid"
          ? "complete"
          : op === "bound"
            ? "Unbound quantified variable"
            : op === "read"
              ? "Invalid safety variable read"
              : "Unbound EXCEPT @",
      positions: [`ir-expression/${op}`],
    })),
  ),
  ...["literal", "data"].map((op) => ({
    id: `ir/leaf/${op}/sentinel`,
    routes: 3,
    classification: "leaf",
    expected: "complete",
    positions: [`ir-expression/${op}`],
  })),
  {
    id: "ir/definition/self-cycle",
    routes: 3,
    classification: "dag",
    expected: "Recursive safety definition graph",
    positions: [
      "ir-definition/definition/body",
      "ir-expression/call/definition",
    ],
  },
  ...[48, 49].map((depth) => ({
    id: `ir/definition/depth/${depth}`,
    routes: 3,
    classification: "depth",
    expected:
      depth === 48 ? "complete" : "Safety expression nesting exceeds 48",
    positions: ["ir-definition/definition/body", "ir-expression/not/argument"],
  })),
].sort((left, right) => left.id.localeCompare(right.id));
const savedIrManifest: readonly { readonly id: string }[] = JSON.parse(
  await readFile(
    "fixtures/tlc-operator-sharing/analysis-ir-manifest.json",
    "utf8",
  ),
);
test("analysis parity IR fixed manifest matches runtime fixtures both ways", () => {
  const expectedIds = new Set(savedIrManifest.map((entry) => entry.id));
  const actualIds = new Set(runtimeIrManifest.map((entry) => entry.id));
  assert.deepEqual(
    [...expectedIds].filter((id) => !actualIds.has(id)),
    [],
    "missing fixed IR fixture IDs",
  );
  assert.deepEqual(
    [...actualIds].filter((id) => !expectedIds.has(id)),
    [],
    "unexpected IR fixture IDs",
  );
  assert.equal(actualIds.size, 343);
  assert.equal(runtimeIrManifest.length, 343);
  assert.deepEqual(runtimeIrManifest, savedIrManifest);
});
