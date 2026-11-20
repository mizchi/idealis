import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import type { PortableCommand } from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyProgram } from "../../packages/idealis/src/safety-program.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

const literal = { op: "literal", value: { type: "bool", value: true } };
const signature = {
  arguments: [literal],
  captures: [],
  parameterCount: 1,
  captureCount: 0,
  localScope: 0,
};
const expression = {
  op: "frame",
  ...signature,
  body: { op: "parameter", slot: 0 },
};
const program = {
  format: "modelt.safety-program",
  version: 2,
  name: "FrameDecoder",
  variables: ["x"],
  definitions: [],
  initial: [],
  next: [],
  invariants: [{ name: "Safe", expression }],
  assumptions: [],
  checkDeadlock: false,
};
const at = (value: unknown) => ({
  ...program,
  invariants: [{ name: "Safe", expression: value }],
});

test("anonymous frame decoder preserves all three v2 boundaries", () => {
  const value = {
    ...program,
    next: [
      {
        name: "Next",
        steps: [{ kind: "frame", ...signature, branches: [] }],
      },
    ],
    properties: [
      {
        name: "P",
        formula: {
          kind: "frame",
          ...signature,
          formula: { kind: "atom", expression: literal },
        },
      },
    ],
  };
  assert.deepEqual(decodeSafetyProgram(value), value);
});

for (const [label, invalid] of [
  ["argument arity", { ...expression, parameterCount: 2 }],
  ["capture arity", { ...expression, captureCount: 1 }],
  ["missing scope", { ...expression, localScope: undefined }],
  ["scope limit", { ...expression, localScope: 41 }],
  ["unknown field", { ...expression, definition: 0 }],
  [
    "unused malformed actual",
    { ...expression, arguments: [{ op: "unknown" }] },
  ],
] as const)
  test(`anonymous frame decoder refuses ${label}`, () => {
    assert.throws(() => decodeSafetyProgram(at(invalid)), PortableCoreError);
  });

for (const [label, value] of [
  ["expression", program],
  [
    "action",
    {
      ...program,
      invariants: [],
      next: [
        {
          name: "Next",
          steps: [{ kind: "frame", ...signature, branches: [] }],
        },
      ],
    },
  ],
  [
    "temporal",
    {
      ...program,
      invariants: [],
      properties: [
        {
          name: "P",
          formula: {
            kind: "frame",
            ...signature,
            formula: { kind: "atom", expression: literal },
          },
        },
      ],
    },
  ],
] as const)
  test(`anonymous frame decoder refuses v1 ${label}`, () => {
    const { definitions: _, ...v1 } = { ...value, version: 1 };
    assert.throws(() => decodeSafetyProgram(v1), PortableCoreError);
  });

const wireIndexMax = 4_294_967_295;
const wireLiteral = { op: "literal", value: { type: "bool", value: true } };
const wireInteger = { op: "literal", value: { type: "int", value: "0" } };
const wireDomain = { kind: "set", values: [wireInteger] };
const wireSignature = {
  arguments: [],
  captures: [],
  parameterCount: 0,
  captureCount: 0,
  localScope: 0,
};
const wireBase = {
  format: "modelt.safety-program",
  version: 2,
  name: "WireBoundary",
  variables: ["x"],
  initial: [
    { name: "Init", steps: [{ kind: "assign", slot: 0, value: wireInteger }] },
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
  invariants: [{ name: "Safe", expression: wireLiteral }],
  assumptions: [],
  definitions: [
    {
      id: 0,
      parameterCount: 0,
      captureCount: 0,
      bodyKind: "expression",
      localScope: 0,
      body: wireLiteral,
    },
  ],
  checkDeadlock: false,
};
type WireFixture = {
  readonly name: string;
  readonly path: readonly (string | number)[];
  readonly value: unknown;
  readonly nativeMaxError?: string;
  readonly tsMaxError?: string;
};
const wireExpression = (
  name: string,
  value: unknown,
  field: string,
  nativeMaxError: string,
  tsMaxError?: string,
): WireFixture => ({
  name,
  value: { ...wireBase, invariants: [{ name: "Safe", expression: value }] },
  path: ["invariants", 0, "expression", field],
  nativeMaxError,
  ...(tsMaxError === undefined ? {} : { tsMaxError }),
});
const wireStep = (
  name: string,
  value: unknown,
  field: string,
  nativeMaxError: string,
): WireFixture => ({
  name,
  value: { ...wireBase, next: [{ name: "Next", steps: [value] }] },
  path: ["next", 0, "steps", 0, field],
  nativeMaxError,
});
const wireFixtures: readonly WireFixture[] = [
  {
    name: "TemporalFormula.call.definition",
    value: {
      ...wireBase,
      properties: [
        {
          name: "P",
          formula: { kind: "call", definition: 0, arguments: [], captures: [] },
        },
      ],
    },
    path: ["properties", 0, "formula", "definition"],
    nativeMaxError: "Unknown safety definition ID",
  },
  wireStep(
    "Step.call.definition",
    { kind: "call", definition: 0, arguments: [], captures: [] },
    "definition",
    "Unknown safety definition ID",
  ),
  ...["parameterCount", "captureCount", "localScope"].map((field) =>
    wireExpression(
      `Expression.frame.${field}`,
      { op: "frame", ...wireSignature, body: wireLiteral },
      field,
      field === "localScope"
        ? "Invalid safety frame local scope"
        : "Invalid safety frame arity or capture count",
      "Invalid compiled frame signature",
    ),
  ),
  wireExpression(
    "Expression.call.definition",
    { op: "call", definition: 0, arguments: [], captures: [] },
    "definition",
    "Unknown safety definition ID",
  ),
  wireExpression(
    "Expression.parameter.slot",
    { op: "parameter", slot: 0 },
    "slot",
    "Invalid safety parameter reference",
  ),
  wireExpression(
    "Expression.capture.slot",
    { op: "capture", slot: 0 },
    "slot",
    "Invalid safety capture reference",
  ),
  wireExpression(
    "Expression.bound.slot",
    { op: "bound", slot: 0 },
    "slot",
    "Unbound quantified variable",
  ),
  wireExpression(
    "Expression.read.slot",
    { op: "read", slot: 0, next: false },
    "slot",
    "Invalid safety variable read",
  ),
  wireExpression(
    "Expression.fold.accumulator",
    {
      op: "fold",
      accumulator: 0,
      initial: wireInteger,
      collection: { op: "tuple", values: [] },
      order: "sequence-left",
      body: wireInteger,
    },
    "accumulator",
    "Invalid fold variable slot",
  ),
  ...["parameterCount", "captureCount", "localScope"].map(
    (field): WireFixture => ({
      ...wireStep(
        `Step.frame.${field}`,
        { kind: "frame", ...wireSignature, branches: [] },
        field,
        field === "localScope"
          ? "Invalid safety frame local scope"
          : "Invalid safety frame arity or capture count",
      ),
      tsMaxError: "Invalid compiled frame signature",
    }),
  ),
  wireStep(
    "Step.bind.slot",
    { kind: "bind", slot: 0, domain: wireDomain },
    "slot",
    "Invalid bound variable slot",
  ),
  wireStep(
    "Step.bind.tupleSize",
    { kind: "bind", slot: 0, tupleSize: 1, domain: wireDomain },
    "tupleSize",
    "Invalid bound variable slot",
  ),
  wireStep(
    "Step.assign.slot",
    { kind: "assign", slot: 0, value: wireInteger },
    "slot",
    "Invalid safety assignment slot",
  ),
  wireStep(
    "Step.choose.slot",
    { kind: "choose", slot: 0, domain: wireDomain },
    "slot",
    "Invalid safety assignment slot",
  ),
  ...["slot", "tupleSize"].map((field): WireFixture => ({
    name: `Binding.${field}`,
    value: {
      ...wireBase,
      invariants: [
        {
          name: "Safe",
          expression: {
            op: "forall",
            bounds: [{ slot: 0, tupleSize: 1, domain: wireDomain }],
            body: wireLiteral,
          },
        },
      ],
    },
    path: ["invariants", 0, "expression", "bounds", 0, field],
    nativeMaxError: "Invalid bound variable slot",
  })),
  ...["id", "parameterCount", "captureCount", "localScope"].map(
    (field): WireFixture => ({
      name: `Definition.${field}`,
      value: wireBase,
      path: ["definitions", 0, field],
      ...(["id", "localScope"].includes(field)
        ? {
            nativeMaxError: "Invalid safety definition ID or local scope",
            tsMaxError: "Invalid compiled definition",
          }
        : {}),
    }),
  ),
  ...["parameterCount", "captureCount", "localScope"].map(
    (field): WireFixture => ({
      name: `TemporalFormula.frame.${field}`,
      value: {
        ...wireBase,
        properties: [
          {
            name: "P",
            formula: {
              kind: "frame",
              ...wireSignature,
              formula: { kind: "atom", expression: wireLiteral },
            },
          },
        ],
      },
      path: ["properties", 0, "formula", field],
      nativeMaxError:
        field === "localScope"
          ? "Invalid safety frame local scope"
          : "Invalid safety frame arity or capture count",
      tsMaxError: "Invalid compiled frame signature",
    }),
  ),
];
const wireAt = (fixture: WireFixture, count: number): unknown => {
  const value: unknown = structuredClone(fixture.value);
  let parent = value;
  for (const key of fixture.path.slice(0, -1)) {
    assert.ok(isRecord(parent) || Array.isArray(parent));
    parent = (parent as Record<string | number, unknown>)[key];
  }
  assert.ok(isRecord(parent) || Array.isArray(parent));
  const key = fixture.path.at(-1);
  assert.notEqual(key, undefined);
  (parent as Record<string | number, unknown>)[key as string | number] = count;
  return value;
};
const wireNative = (value: unknown): unknown => {
  const result = spawnSync(
    "crates/idealis-core/target/release/idealis-core",
    [],
    {
      input: `${JSON.stringify({ version: 1, request: { kind: "check-safety", program: value, bounds: { maxDepth: 8, maxStates: 100, maxWork: 10000 } } })}\n`,
      encoding: "utf8",
      timeout: 1000,
      maxBuffer: 1024 * 1024,
    },
  );
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
};
const wireCore = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const wireWasm = (value: unknown): unknown => {
  try {
    return wireCore.request({
      version: 1,
      request: {
        kind: "check-safety",
        program: value,
        bounds: { maxDepth: 8, maxStates: 100, maxWork: 10000 },
      },
    } as PortableCommand);
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error", code: error.code, message: error.message };
  }
};

test("analysis parity wire field inventory has 27 distinct declarations", () => {
  assert.equal(wireFixtures.length, 27);
  assert.equal(new Set(wireFixtures.map((fixture) => fixture.name)).size, 27);
});
for (const fixture of wireFixtures) {
  test(`analysis parity wire TS decoder max: ${fixture.name}`, () => {
    const value = wireAt(fixture, wireIndexMax);
    if (fixture.tsMaxError !== undefined) {
      assert.throws(() => decodeSafetyProgram(value), {
        code: "protocol",
        message: fixture.tsMaxError,
      });
    } else {
      assert.deepEqual(decodeSafetyProgram(value), value);
    }
  });
  test(`analysis parity wire TS decoder max+1: ${fixture.name}`, () => {
    assert.throws(
      () => decodeSafetyProgram(wireAt(fixture, wireIndexMax + 1)),
      { code: "protocol", message: "Invalid compiled program count" },
    );
  });
  test(`analysis parity wire raw max semantic boundary: ${fixture.name}`, () => {
    const value = wireAt(fixture, wireIndexMax);
    const native = wireNative(value);
    assert.deepEqual(wireWasm(value), native);
    assert.ok(isRecord(native));
    if (fixture.nativeMaxError !== undefined) {
      assert.deepEqual(native, {
        kind: "error",
        code: "protocol",
        message: fixture.nativeMaxError,
      });
    } else {
      assert.equal(native.kind, "safety");
      assert.ok(isRecord(native.result));
      assert.equal(native.result.status, "complete");
      assert.equal(native.result.states, 1);
      assert.equal(native.result.generated, 2);
    }
  });
  for (const [backend, run] of [
    ["native", wireNative],
    ["Wasm", wireWasm],
  ] as const) {
    test(`analysis parity wire raw ${backend} max+1: ${fixture.name}`, () => {
      const result = run(wireAt(fixture, wireIndexMax + 1));
      assert.ok(isRecord(result));
      assert.equal(
        result.kind,
        "error",
        `${fixture.name} accepted an out-of-range wire index`,
      );
      assert.equal(result.code, "protocol");
    });
  }
}

for (const [name, value] of [
  ["negative", -1],
  ["fraction", 0.5],
  ["unsafe", Number.MAX_SAFE_INTEGER + 1],
] as const) {
  test(`analysis parity wire TS decoder refuses ${name}`, () => {
    for (const fixture of wireFixtures) {
      assert.throws(() => decodeSafetyProgram(wireAt(fixture, value)), {
        code: "protocol",
        message: "Invalid compiled program count",
      });
    }
  });
}
for (const space of [
  "definition",
  "expression",
  "action",
  "temporal",
] as const) {
  test(`analysis parity wire localScope 40/41: ${space}`, () => {
    const fixture = wireFixtures.find(
      (entry) =>
        entry.name ===
        (space === "definition"
          ? "Definition.localScope"
          : space === "expression"
            ? "Expression.frame.localScope"
            : space === "action"
              ? "Step.frame.localScope"
              : "TemporalFormula.frame.localScope"),
    );
    assert.ok(fixture);
    const valid = wireAt(fixture, 40);
    assert.deepEqual(decodeSafetyProgram(valid), valid);
    const native = wireNative(valid);
    assert.deepEqual(wireWasm(valid), native);
    assert.ok(isRecord(native));
    assert.equal(native.kind, "safety");
    const invalid = wireAt(fixture, 41);
    assert.throws(() => decodeSafetyProgram(invalid), PortableCoreError);
    const refused = wireNative(invalid);
    assert.deepEqual(wireWasm(invalid), refused);
    assert.ok(isRecord(refused));
    assert.equal(refused.kind, "error");
    assert.equal(refused.code, "protocol");
  });
}

const wireRoot = (expression: unknown): unknown => ({
  ...wireBase,
  invariants: [{ name: "Safe", expression }],
});
const wireControls: readonly {
  readonly name: string;
  readonly valid: unknown;
  readonly invalid: unknown;
}[] = [
  ...["parameter", "capture"].map((op) => {
    const callSignature = {
      arguments: op === "parameter" ? [wireLiteral] : [],
      captures: op === "capture" ? [wireLiteral] : [],
      parameterCount: op === "parameter" ? 1 : 0,
      captureCount: op === "capture" ? 1 : 0,
      localScope: 0,
    };
    return {
      name: `${op} slot`,
      valid: wireRoot({ op: "frame", ...callSignature, body: { op, slot: 0 } }),
      invalid: wireRoot({
        op: "frame",
        ...callSignature,
        body: { op, slot: 1 },
      }),
    };
  }),
  ...["parameterCount", "captureCount"].map((field) => ({
    name: `${field} arity`,
    valid: wireRoot({ op: "frame", ...wireSignature, body: wireLiteral }),
    invalid: wireRoot({
      op: "frame",
      ...wireSignature,
      [field]: 1,
      body: wireLiteral,
    }),
  })),
  {
    name: "read slot",
    valid: wireRoot({
      op: "eq",
      left: { op: "read", slot: 0, next: false },
      right: wireInteger,
    }),
    invalid: wireRoot({
      op: "eq",
      left: { op: "read", slot: 1, next: false },
      right: wireInteger,
    }),
  },
  ...["assign", "choose", "bind"].map((kind) => ({
    name: `${kind} slot`,
    valid: {
      ...wireBase,
      next: [
        {
          name: "Next",
          steps: [
            {
              kind,
              slot: 0,
              ...(kind === "assign"
                ? { value: wireInteger }
                : { domain: wireDomain }),
            },
            ...(kind === "bind" ? (wireBase.next[0]?.steps ?? []) : []),
          ],
        },
      ],
    },
    invalid: {
      ...wireBase,
      next: [
        {
          name: "Next",
          steps: [
            {
              kind,
              slot: 1,
              ...(kind === "assign"
                ? { value: wireInteger }
                : { domain: wireDomain }),
            },
          ],
        },
      ],
    },
  })),
  ...["binding slot", "bound slot"].map((name) => ({
    name,
    valid: wireRoot({
      op: "forall",
      bounds: [{ slot: 0, domain: { kind: "set", values: [wireLiteral] } }],
      body: { op: "bound", slot: 0 },
    }),
    invalid: wireRoot({
      op: "forall",
      bounds: [
        {
          slot: name === "binding slot" ? 1 : 0,
          domain: { kind: "set", values: [wireLiteral] },
        },
      ],
      body: { op: "bound", slot: name === "bound slot" ? 1 : 0 },
    }),
  })),
];
for (const control of wireControls) {
  test(`analysis parity wire valid/invalid ${control.name}`, () => {
    const valid = wireNative(control.valid);
    assert.deepEqual(wireWasm(control.valid), valid);
    assert.ok(isRecord(valid));
    assert.equal(valid.kind, "safety");
    assert.ok(isRecord(valid.result));
    assert.equal(valid.result.status, "complete");
    const invalid = wireNative(control.invalid);
    assert.deepEqual(wireWasm(control.invalid), invalid);
    assert.ok(isRecord(invalid));
    assert.equal(invalid.kind, "error");
    assert.equal(invalid.code, "protocol");
  });
}
