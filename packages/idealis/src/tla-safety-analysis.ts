import type { Module } from "tla-ts";
import type {
  SafetyBinding,
  SafetyBranch,
  SafetyDefinition,
  SafetyDomain,
  SafetyExpressionV2,
  SafetyPredicate,
  SafetyProgramV2,
  SafetyStep,
  TemporalFormula,
  TemporalProperty,
} from "./contracts.js";

type Source = Module["definitions"][number]["body"];
type Definition = Module["definitions"][number];
type Variant<T, K extends PropertyKey, V> =
  T extends Record<K, infer U> ? (V extends U ? T : never) : never;
type SourceOf<K extends Source["kind"]> = Variant<Source, "kind", K>;
type ExpressionOf<K extends SafetyExpressionV2["op"]> = Variant<
  SafetyExpressionV2,
  "op",
  K
>;
type TemporalOf<K extends TemporalFormula<2>["kind"]> = Variant<
  TemporalFormula<2>,
  "kind",
  K
>;
type StepOf<K extends SafetyStep<2>["kind"]> = Variant<
  SafetyStep<2>,
  "kind",
  K
>;
type DomainOf<K extends SafetyDomain<2>["kind"]> = Variant<
  SafetyDomain<2>,
  "kind",
  K
>;
type Bound = SourceOf<"Forall">["bounds"][number];
type ExceptPath = SourceOf<"Except">["updates"][number]["path"][number];

export interface AnalysisRole {
  readonly space: string;
  readonly constructor: string;
  readonly operatorClass: string;
  readonly position: string;
  readonly role: string;
  readonly scope: string;
  readonly forwarding: {
    readonly action: string;
    readonly membership: string;
  };
}

export type AnalysisNode =
  | { readonly space: "source"; readonly value: Source }
  | { readonly space: "source-definition"; readonly value: Definition }
  | { readonly space: "source-bound"; readonly value: Bound }
  | { readonly space: "source-except-path"; readonly value: ExceptPath }
  | { readonly space: "ir-expression"; readonly value: SafetyExpressionV2 }
  | { readonly space: "ir-domain"; readonly value: SafetyDomain<2> }
  | { readonly space: "ir-binding"; readonly value: SafetyBinding<2> }
  | { readonly space: "ir-branch"; readonly value: SafetyBranch<2> }
  | { readonly space: "ir-step"; readonly value: SafetyStep<2> }
  | { readonly space: "ir-definition"; readonly value: SafetyDefinition }
  | { readonly space: "ir-temporal"; readonly value: TemporalFormula<2> }
  | { readonly space: "ir-program"; readonly value: SafetyProgramV2 }
  | { readonly space: "ir-predicate"; readonly value: SafetyPredicate<2> }
  | { readonly space: "ir-property"; readonly value: TemporalProperty<2> }
  | { readonly space: "ir-list"; readonly value: readonly unknown[] }
  | { readonly space: "ir-call-id"; readonly value: number };

export interface AnalysisEdge {
  readonly node: AnalysisNode;
  readonly position: string;
  readonly role: string;
  readonly scope: string;
  readonly forwarding: { readonly action: string; readonly membership: string };
  readonly depth: number;
  readonly path: readonly (string | number)[];
}
type Space = AnalysisNode["space"];
interface EdgeSpec {
  readonly type: "edge";
  readonly space: Space;
  readonly role: string;
  readonly scope: string;
  readonly forwarding: { readonly action: string; readonly membership: string };
  readonly depth: number;
}
type Field =
  | "annotation"
  | EdgeSpec
  | { readonly each: Field }
  | { readonly fields: Readonly<Record<string, Field>> }
  | { readonly tuple: readonly Field[] }
  | {
      readonly tag: string;
      readonly variants: Readonly<Record<string, Field>>;
    };
type Descriptor<T> = { readonly [K in keyof T]-?: Field };
const a = "annotation";
const edge = (
  space: Space,
  role = "value",
  scope = "caller",
  action = "stop",
  membership = "stop",
  depth = 1,
): EdgeSpec => ({
  type: "edge",
  space,
  role,
  scope,
  forwarding: { action, membership },
  depth,
});
const source = (
  role = "value",
  scope = "caller",
  action = "stop",
  membership = "stop",
) => edge("source", role, scope, action, membership, 0);
const expr = (
  role = "value",
  scope = "caller",
  action = "stop",
  membership = "stop",
  depth = 1,
) => edge("ir-expression", role, scope, action, membership, depth);
const each = (value: Field): Field => ({ each: value });
const fields = <T>(value: Descriptor<T>) => ({ fields: value });
const temporal = (role = "temporal-body", scope = "caller") =>
  edge("ir-temporal", role, scope, "stop", "stop", 0);
const domain = (role = "binder-domain") =>
  edge("ir-domain", role, "caller", "stop", "stop", 0);
const binding = edge(
  "ir-binding",
  "binder-domain",
  "caller",
  "stop",
  "stop",
  0,
);
const branch = (role = "action-branch", scope = "caller", action = "start") =>
  edge("ir-branch", role, scope, action, "stop", 0);
const result = source("result", "caller", "inherit", "inherit");
const boundFields = {
  pattern: a,
  set: source("binder-domain"),
} satisfies Descriptor<Bound>;
const sourceBounds = each(fields<Bound>(boundFields));
const sourceBody = (action = "stop") => source("binder-body", "binder", action);
const caseArmFields = (condition: Field, body: Field) =>
  fields<SourceOf<"Case">["arms"][number]>({ condition, body });
const recordFields = (value: Field) =>
  each(
    fields<SourceOf<"Record">["fields"][number]>({
      name: a,
      value,
      location: a,
    }),
  );
const sourcePathFields = {
  Field: fields<Variant<ExceptPath, "kind", "Field">>({ kind: a, name: a }),
  Index: fields<Variant<ExceptPath, "kind", "Index">>({
    kind: a,
    args: each(source("EXCEPT-key")),
  }),
} satisfies {
  readonly [K in ExceptPath["kind"]]: {
    readonly fields: Descriptor<Variant<ExceptPath, "kind", K>>;
  };
};
const sourcePath: Field = { tag: "kind", variants: sourcePathFields };
const sourceUpdate = fields<SourceOf<"Except">["updates"][number]>({
  path: each(sourcePath),
  value: source("EXCEPT-replacement", "old"),
});
const sourceSelector = {
  tag: "kind",
  variants: {
    Name: fields<
      Variant<SourceOf<"Subexpression">["selectors"][number], "kind", "Name">
    >({ kind: a, name: a, args: each(source()) }),
    Operand: fields<
      Variant<SourceOf<"Subexpression">["selectors"][number], "kind", "Operand">
    >({ kind: a, operand: a }),
    Bound: fields<
      Variant<SourceOf<"Subexpression">["selectors"][number], "kind", "Bound">
    >({ kind: a, args: each(source()) }),
    At: fields<
      Variant<SourceOf<"Subexpression">["selectors"][number], "kind", "At">
    >({ kind: a }),
    Colon: fields<
      Variant<SourceOf<"Subexpression">["selectors"][number], "kind", "Colon">
    >({ kind: a }),
  },
} satisfies {
  readonly tag: "kind";
  readonly variants: {
    readonly [K in SourceOf<"Subexpression">["selectors"][number]["kind"]]: {
      readonly fields: Descriptor<
        Variant<SourceOf<"Subexpression">["selectors"][number], "kind", K>
      >;
    };
  };
};
const sourceDescriptors = {
  Number: { kind: a, value: a, text: a, location: a },
  Decimal: { kind: a, text: a, location: a },
  String: { kind: a, value: a, location: a },
  Identifier: {
    kind: a,
    name: a,
    spelling: a,
    selectorRanges: a,
    location: a,
    resolution: a,
  },
  Apply: {
    kind: a,
    operator: a,
    spelling: a,
    args: each(source("call-actual", "caller", "summary", "summary")),
    argumentCounts: a,
    argumentsRange: a,
    selectorRanges: a,
    argumentRanges: a,
    location: a,
    resolution: a,
  },
  OperatorReference: { kind: a, operator: a, spelling: a, location: a },
  Lambda: {
    kind: a,
    params: a,
    paramRanges: a,
    body: source(),
    operator: a,
    location: a,
  },
  And: {
    kind: a,
    operands: each(source("action-body", "caller", "inherit")),
    isBulleted: a,
    prefixEnds: a,
    location: a,
  },
  Or: {
    kind: a,
    operands: each(source("action-body", "caller", "inherit")),
    isBulleted: a,
    prefixEnds: a,
    location: a,
  },
  Prime: { kind: a, expression: source("prime-argument"), location: a },
  Unchanged: {
    kind: a,
    expression: source("generative-target", "caller", "target-in-action"),
    location: a,
  },
  Enabled: {
    kind: a,
    expression: source("action-body", "caller", "start"),
    location: a,
  },
  If: {
    kind: a,
    condition: source("condition"),
    consequent: result,
    alternative: result,
    location: a,
  },
  Case: {
    kind: a,
    arms: each(caseArmFields(source("condition"), result)),
    other: result,
    otherRange: a,
    location: a,
  },
  Let: {
    kind: a,
    definitions: each(
      edge(
        "source-definition",
        "definition-body",
        "declaration",
        "stop",
        "stop",
        0,
      ),
    ),
    instances: a,
    body: result,
    location: a,
  },
  Forall: { kind: a, bounds: sourceBounds, body: sourceBody(), location: a },
  Exists: {
    kind: a,
    bounds: sourceBounds,
    body: sourceBody("inherit"),
    location: a,
  },
  UnboundedQuantifier: {
    kind: a,
    quantifier: a,
    names: a,
    nameRanges: a,
    isTemporal: a,
    body: source(),
    location: a,
  },
  Choose: {
    kind: a,
    pattern: a,
    set: source("binder-domain"),
    body: sourceBody(),
    location: a,
  },
  SetEnumeration: { kind: a, elements: each(source()), location: a },
  SetFilter: {
    kind: a,
    bound: fields<Bound>(boundFields),
    predicate: sourceBody(),
    location: a,
  },
  SetMap: {
    kind: a,
    expression: sourceBody(),
    bounds: sourceBounds,
    location: a,
  },
  Tuple: { kind: a, elements: each(source()), location: a },
  FunctionConstruction: {
    kind: a,
    bounds: sourceBounds,
    body: sourceBody(),
    location: a,
    isRecursive: a,
  },
  FunctionSet: {
    kind: a,
    domain: source(),
    range: source("membership-result", "caller", "stop", "inherit"),
    location: a,
  },
  Record: { kind: a, fields: recordFields(source()), location: a },
  RecordSet: {
    kind: a,
    fields: recordFields(
      source("membership-result", "caller", "stop", "inherit"),
    ),
    location: a,
  },
  Except: {
    kind: a,
    function: source("EXCEPT-base"),
    updates: each(sourceUpdate),
    location: a,
  },
  ExceptAt: { kind: a, location: a },
  FunctionApplication: {
    kind: a,
    function: source(),
    args: each(source("call-actual")),
    location: a,
  },
  Bracket: { kind: a, args: each(source()), location: a },
  FieldAccess: { kind: a, record: source(), field: a, location: a },
  Product: { kind: a, factors: each(source()), location: a },
  Always: { kind: a, expression: source("temporal-body"), location: a },
  Eventually: { kind: a, expression: source("temporal-body"), location: a },
  BoxAction: {
    kind: a,
    action: source("action-body", "caller", "inherit"),
    subscript: source("subscript", "caller", "target-in-action"),
    location: a,
  },
  AngleAction: {
    kind: a,
    action: source("action-body", "caller", "inherit"),
    subscript: source("subscript", "caller", "target-in-action"),
    location: a,
  },
  WeakFairness: {
    kind: a,
    subscript: source("subscript", "caller", "target"),
    action: source("action-body", "caller", "inherit"),
    enabled: a,
    location: a,
  },
  StrongFairness: {
    kind: a,
    subscript: source("subscript", "caller", "target"),
    action: source("action-body", "caller", "inherit"),
    enabled: a,
    location: a,
  },
  Label: {
    kind: a,
    name: a,
    params: a,
    paramRanges: a,
    head: a,
    body: result,
    location: a,
  },
  Subexpression: {
    kind: a,
    selectors: each(sourceSelector),
    selectorRanges: a,
    argumentRanges: a,
    location: a,
  },
} satisfies { readonly [K in Source["kind"]]: Descriptor<SourceOf<K>> };

const sourceDefinitionDescriptors = {
  Operator: {
    kind: a,
    name: a,
    params: a,
    body: source("definition-body", "declaration"),
    bodyRange: a,
    head: a,
    isLocal: a,
    isRecursive: a,
    location: a,
  },
  Function: {
    kind: a,
    name: a,
    bounds: each(
      fields<Bound>({
        pattern: a,
        set: source("definition-body", "declaration"),
      }),
    ),
    body: source("definition-body", "declaration"),
    isLocal: a,
    isRecursive: a,
    isSelfReferential: a,
    location: a,
  },
} satisfies {
  readonly [K in Definition["kind"]]: Descriptor<
    Variant<Definition, "kind", K>
  >;
};

const frameFields = {
  arguments: each(expr("frame-actual")),
  captures: each(expr("frame-capture")),
  parameterCount: a,
  captureCount: a,
  localScope: a,
} as const;
const unary = (argument = expr()) => ({ op: a, argument }) as const;
const binary = (left = expr(), right = expr()) =>
  ({ op: a, left, right }) as const;
const irRecord = (value: Field) => each({ tuple: [a, value] });
const irUpdate = fields<ExpressionOf<"update">["updates"][number]>({
  path: each(expr("EXCEPT-key")),
  value: expr("EXCEPT-replacement", "old"),
});
const irBinder = (action = "stop") =>
  ({
    op: a,
    bounds: each({ ...binding, depth: 1 }),
    body: expr("binder-body", "binder", action),
  }) as const;
const expressionDescriptors = {
  data: { op: a, value: a },
  bound: { op: a, slot: a },
  old: { op: a },
  "unmatched-case": { op: a, location: a },
  set: { op: a, values: each(expr()) },
  tuple: { op: a, values: each(expr()) },
  record: { op: a, fields: irRecord(expr()) },
  "record-set": {
    op: a,
    fields: irRecord(expr("membership-result", "caller", "stop", "inherit")),
  },
  at: { op: a, target: expr(), key: expr() },
  update: { op: a, target: expr("EXCEPT-base"), updates: each(irUpdate) },
  forall: irBinder(),
  exists: irBinder("inherit"),
  function: irBinder(),
  filter: irBinder(),
  "map-set": irBinder(),
  fold: {
    op: a,
    accumulator: a,
    initial: expr(),
    collection: expr(),
    order: a,
    body: expr("binder-body", "binder"),
  },
  choose: {
    op: a,
    binding: { ...binding, depth: 1 },
    body: expr("binder-body", "binder"),
  },
  "sequence-set": unary(expr("membership-result", "caller", "stop", "inherit")),
  domain: unary(),
  "variant-tag": unary(),
  cardinality: unary(),
  len: unary(),
  head: unary(),
  tail: unary(),
  powerset: unary(),
  union: unary(),
  cup: binary(),
  cap: binary(),
  difference: binary(),
  "subset-eq": binary(),
  append: binary(),
  concat: binary(),
  "function-set": binary(
    expr(),
    expr("membership-result", "caller", "stop", "inherit"),
  ),
  "function-in": binary(),
  "sequence-in": binary(),
  subseq: { op: a, sequence: expr(), from: expr(), to: expr() },
  "select-seq": {
    op: a,
    slot: a,
    sequence: expr(),
    body: expr("binder-body", "binder"),
  },
  "sort-seq": {
    op: a,
    slot: a,
    sequence: expr(),
    body: expr("binder-body", "binder"),
  },
  assert: { op: a, condition: expr(), message: expr() },
  enabled: {
    op: a,
    branches: each({ ...branch(), depth: 1 }),
    subscript: expr("subscript", "caller", "target", "stop"),
  },
  literal: { op: a, value: a },
  read: { op: a, slot: a, next: a },
  not: unary(),
  neg: unary(),
  prime: unary(expr("prime-argument")),
  add: binary(),
  sub: binary(),
  mul: binary(),
  div: binary(),
  mod: binary(),
  pow: binary(),
  eq: binary(),
  ne: binary(),
  lt: binary(),
  le: binary(),
  gt: binary(),
  ge: binary(),
  and: binary(),
  or: binary(),
  implies: binary(),
  if: {
    op: a,
    condition: expr("condition"),
    consequent: expr("result", "caller", "inherit", "inherit"),
    alternative: expr("result", "caller", "inherit", "inherit"),
  },
  in: {
    op: a,
    value: expr("membership-operand"),
    domain: edge(
      "ir-domain",
      "membership-result",
      "caller",
      "stop",
      "start",
      1,
    ),
  },
  frame: { op: a, ...frameFields, body: expr("frame-body", "frame") },
  call: {
    op: a,
    definition: edge(
      "ir-call-id",
      "call-edge",
      "definition",
      "stop",
      "stop",
      0,
    ),
    arguments: each(expr("call-actual")),
    captures: each(expr("call-capture")),
  },
  parameter: { op: a, slot: a },
  capture: { op: a, slot: a },
} satisfies {
  readonly [K in SafetyExpressionV2["op"]]: Descriptor<ExpressionOf<K>>;
};
const domainDescriptors = {
  value: { kind: a, value: expr("binder-domain", "caller", "stop", "stop", 0) },
  range: {
    kind: a,
    lower: expr("binder-domain", "caller", "stop", "stop", 0),
    upper: expr("binder-domain", "caller", "stop", "stop", 0),
  },
  set: {
    kind: a,
    values: each(expr("binder-domain", "caller", "stop", "stop", 0)),
  },
} satisfies {
  readonly [K in SafetyDomain<2>["kind"]]: Descriptor<DomainOf<K>>;
};
const bindingDescriptor = {
  slot: a,
  tupleSize: a,
  domain: domain(),
} satisfies Descriptor<SafetyBinding<2>>;
const branchDescriptor = {
  name: a,
  steps: each(edge("ir-step", "action-step", "caller", "inherit", "stop", 0)),
} satisfies Descriptor<SafetyBranch<2>>;
const stepDescriptors = {
  call: {
    kind: a,
    definition: edge(
      "ir-call-id",
      "call-edge",
      "definition",
      "stop",
      "stop",
      0,
    ),
    arguments: each(expr("call-actual")),
    captures: each(expr("call-capture")),
  },
  bind: { kind: a, slot: a, domain: domain("value"), tupleSize: a },
  guard: { kind: a, expression: expr("value", "caller", "stop", "stop", 0) },
  assign: {
    kind: a,
    slot: a,
    value: expr("value", "caller", "stop", "stop", 0),
  },
  choose: { kind: a, slot: a, domain: domain("value") },
  frame: {
    kind: a,
    ...frameFields,
    branches: each({
      ...branch("action-frame-body", "frame", "inherit"),
      depth: 1,
    }),
  },
} satisfies { readonly [K in SafetyStep<2>["kind"]]: Descriptor<StepOf<K>> };
const definitionDescriptor = {
  recursive: a,
  id: a,
  parameterCount: a,
  captureCount: a,
  bodyKind: a,
  localScope: a,
  body: expr("definition-body", "frame", "stop", "stop", 0),
} satisfies Descriptor<SafetyDefinition>;
const actionDefinitionDescriptor = {
  ...definitionDescriptor,
  body: each(branch("definition-body", "frame", "stop")),
} satisfies Descriptor<SafetyDefinition>;
const temporalDefinitionDescriptor = {
  ...definitionDescriptor,
  body: { ...temporal("definition-body", "frame"), depth: 0 },
} satisfies Descriptor<SafetyDefinition>;
const temporalDescriptors = {
  call: stepDescriptors.call,
  atom: { kind: a, expression: { ...expr(), depth: 0 } },
  forall: {
    kind: a,
    bounds: each(binding),
    formula: temporal("temporal-body", "binder"),
  },
  exists: {
    kind: a,
    bounds: each(binding),
    formula: temporal("temporal-body", "binder"),
  },
  not: { kind: a, formula: temporal() },
  always: { kind: a, formula: temporal() },
  eventually: { kind: a, formula: temporal() },
  and: { kind: a, left: temporal(), right: temporal() },
  or: { kind: a, left: temporal(), right: temporal() },
  frame: {
    kind: a,
    ...frameFields,
    formula: { ...temporal("frame-body", "frame"), depth: 1 },
  },
} satisfies {
  readonly [K in TemporalFormula<2>["kind"]]: Descriptor<TemporalOf<K>>;
};
const predicateDescriptor = {
  name: a,
  expression: expr("value", "caller", "stop", "stop", 0),
} satisfies Descriptor<SafetyPredicate<2>>;
const propertyDescriptor = {
  name: a,
  formula: edge("ir-temporal", "temporal-body", "caller", "stop", "stop", 0),
} satisfies Descriptor<TemporalProperty<2>>;
const programDescriptor = {
  symmetry: a,
  format: a,
  version: a,
  name: a,
  variables: a,
  checkDeadlock: a,
  initial: each(branch()),
  next: each(branch()),
  definitions: each(
    edge("ir-definition", "definition-body", "declaration", "stop", "stop", 0),
  ),
  invariants: each(fields<SafetyPredicate<2>>(predicateDescriptor)),
  assumptions: each(fields<SafetyPredicate<2>>(predicateDescriptor)),
  initialProperties: each(fields<SafetyPredicate<2>>(predicateDescriptor)),
  actionProperties: each(fields<SafetyPredicate<2>>(predicateDescriptor)),
  constraints: each(fields<SafetyPredicate<2>>(predicateDescriptor)),
  actionConstraints: each(fields<SafetyPredicate<2>>(predicateDescriptor)),
  properties: each(fields<TemporalProperty<2>>(propertyDescriptor)),
  fairness: each(
    edge("ir-temporal", "temporal-body", "caller", "stop", "stop", 0),
  ),
} satisfies Descriptor<SafetyProgramV2>;

const neverNode = (value: never): never => {
  throw new Error(`Unclassified analysis node: ${String(value)}`);
};
const applyArguments = (
  operator: string,
  arity: number,
): readonly EdgeSpec[] => {
  const value = source();
  const args = Array.from({ length: arity }, () => value);
  if (["\\in", "∈", "\\notin", "∉"].includes(operator) && arity === 2) {
    return [
      source(
        "membership-operand",
        "caller",
        ["\\in", "∈"].includes(operator) ? "target-if-identifier" : "stop",
      ),
      source("membership-result", "caller", "stop", "start"),
    ];
  }
  if (operator === "=" && arity === 2)
    return [
      source("assignment-target", "caller", "target-if-identifier"),
      source("assignment-target", "caller", "target-if-identifier"),
    ];
  if (operator === "Seq" && arity === 1)
    return [source("membership-result", "caller", "stop", "inherit")];
  if (["ApaFoldSet", "ApaFoldSeqLeft"].includes(operator) && arity === 3)
    return [source("operator-reference", "declaration"), value, value];
  if (operator === "~>" && arity === 2)
    return [source("temporal-body"), source("temporal-body")];
  if (
    operator !== "user call representative" &&
    applyArities.some(([name, count]) => name === operator && count === arity)
  )
    return args;
  return args.map(() => source("call-actual", "caller", "summary", "summary"));
};
const sourceDescriptor = (value: Source): Readonly<Record<string, Field>> => {
  switch (value.kind) {
    case "Apply":
      return {
        ...sourceDescriptors.Apply,
        args: { tuple: applyArguments(value.operator, value.args.length) },
      };
    case "Number":
    case "Decimal":
    case "String":
    case "Identifier":
    case "OperatorReference":
    case "Lambda":
    case "And":
    case "Or":
    case "Prime":
    case "Unchanged":
    case "Enabled":
    case "If":
    case "Case":
    case "Let":
    case "Forall":
    case "Exists":
    case "UnboundedQuantifier":
    case "Choose":
    case "SetEnumeration":
    case "SetFilter":
    case "SetMap":
    case "Tuple":
    case "FunctionConstruction":
    case "FunctionSet":
    case "Record":
    case "RecordSet":
    case "Except":
    case "ExceptAt":
    case "FunctionApplication":
    case "Bracket":
    case "FieldAccess":
    case "Product":
    case "Always":
    case "Eventually":
    case "BoxAction":
    case "AngleAction":
    case "WeakFairness":
    case "StrongFairness":
    case "Label":
    case "Subexpression":
      return sourceDescriptors[value.kind];
    default:
      return neverNode(value);
  }
};
const descriptorFor = (
  node: AnalysisNode,
): Readonly<Record<string, Field>> | undefined => {
  switch (node.space) {
    case "source":
      return sourceDescriptor(node.value);
    case "source-definition":
      return sourceDefinitionDescriptors[node.value.kind];
    case "source-bound":
      return { pattern: a, set: source() } satisfies Descriptor<Bound>;
    case "source-except-path":
      return sourcePathFields[node.value.kind].fields;
    case "ir-expression":
      return expressionDescriptors[node.value.op];
    case "ir-domain":
      return domainDescriptors[node.value.kind];
    case "ir-binding":
      return bindingDescriptor;
    case "ir-branch":
      return branchDescriptor;
    case "ir-step":
      return stepDescriptors[node.value.kind];
    case "ir-definition":
      return node.value.bodyKind === "action"
        ? actionDefinitionDescriptor
        : node.value.bodyKind === "temporal"
          ? temporalDefinitionDescriptor
          : definitionDescriptor;
    case "ir-temporal":
      return temporalDescriptors[node.value.kind];
    case "ir-program":
      return programDescriptor;
    case "ir-predicate":
      return predicateDescriptor;
    case "ir-property":
      return propertyDescriptor;
    case "ir-list":
    case "ir-call-id":
      return undefined;
    default:
      return neverNode(node);
  }
};
const pathText = (path: readonly (string | number)[]): string =>
  path
    .map((part, index) =>
      typeof part === "number" ? `[${part}]` : `${index ? "." : ""}${part}`,
    )
    .join("");
const asNode = (space: Space, value: unknown): AnalysisNode =>
  ({ space, value }) as AnalysisNode;
export const analysisChildren = (
  node: AnalysisNode,
): readonly AnalysisEdge[] => {
  const result: AnalysisEdge[] = [];
  const scan = (
    spec: Field,
    value: unknown,
    path: readonly (string | number)[],
  ): void => {
    if (spec === a || value === undefined || value === null) return;
    if ("type" in spec) {
      result.push({
        node: asNode(spec.space, value),
        position: pathText(path),
        role: spec.role,
        scope: spec.scope,
        forwarding: spec.forwarding,
        depth: spec.depth,
        path,
      });
    } else if ("each" in spec) {
      (value as readonly unknown[]).forEach((child, index) => {
        scan(spec.each, child, [...path, index]);
      });
    } else if ("tuple" in spec) {
      spec.tuple.forEach((part, index) => {
        scan(part, (value as readonly unknown[])[index], [...path, index]);
      });
    } else if ("fields" in spec) {
      for (const [key, part] of Object.entries(spec.fields))
        scan(part, (value as Record<string, unknown>)[key], [...path, key]);
    } else {
      const tag = (value as Record<string, unknown>)[spec.tag];
      const variant = spec.variants[String(tag)];
      if (variant) scan(variant, value, path);
    }
  };
  if (node.space === "ir-list") {
    node.value.forEach((value, index) => {
      const child = analysisView(value);
      if (child)
        result.push({
          node: child,
          position: `[${index}]`,
          role: "value",
          scope: "caller",
          forwarding: { action: "stop", membership: "stop" },
          depth: 0,
          path: [index],
        });
    });
  } else {
    const descriptor = descriptorFor(node);
    if (descriptor) scan({ fields: descriptor }, node.value, []);
  }
  return result;
};

export const analysisView = (value: unknown): AnalysisNode | undefined => {
  if (Array.isArray(value)) return asNode("ir-list", value);
  if (typeof value !== "object" || value === null) return undefined;
  const input = value as Record<string, unknown>;
  if (
    typeof input.op === "string" &&
    Object.hasOwn(expressionDescriptors, input.op)
  )
    return asNode("ir-expression", value);
  if (input.format === "modelt.safety-program")
    return asNode("ir-program", value);
  if (typeof input.kind === "string") {
    if (input.kind === "frame")
      return asNode("branches" in input ? "ir-step" : "ir-temporal", value);
    if (Object.hasOwn(stepDescriptors, input.kind))
      return asNode("ir-step", value);
    if (Object.hasOwn(temporalDescriptors, input.kind))
      return asNode("ir-temporal", value);
    if (Object.hasOwn(domainDescriptors, input.kind))
      return asNode("ir-domain", value);
  }
  if ("bodyKind" in input) return asNode("ir-definition", value);
  if ("slot" in input && "domain" in input) return asNode("ir-binding", value);
  if ("steps" in input) return asNode("ir-branch", value);
  if ("name" in input && "expression" in input)
    return asNode("ir-predicate", value);
  if ("name" in input && "formula" in input)
    return asNode("ir-property", value);
  return undefined;
};
const replacePath = (
  value: unknown,
  path: readonly (string | number)[],
  replacement: unknown,
): unknown => {
  const [head, ...tail] = path;
  if (head === undefined) return replacement;
  if (Array.isArray(value))
    return value.map((entry, index) =>
      index === head ? replacePath(entry, tail, replacement) : entry,
    );
  const input = value as Record<string, unknown>;
  return { ...input, [head]: replacePath(input[head], tail, replacement) };
};
export const mapAnalysisChildren = (
  node: AnalysisNode,
  transform: (edge: AnalysisEdge) => unknown,
): unknown => {
  let value: unknown = node.value;
  for (const child of analysisChildren(node))
    value = replacePath(value, child.path, transform(child));
  return value;
};

const applyArities: readonly (readonly [string, number])[] = [
  ...[
    "~",
    "\\lnot",
    "\\neg",
    "¬",
    "-",
    "DOMAIN",
    "Cardinality",
    "Len",
    "Head",
    "Tail",
    "SUBSET",
    "UNION",
  ].map((name) => [name, 1] as const),
  ...[
    "+",
    "-",
    "*",
    "#",
    "/=",
    "≠",
    "<",
    "<=",
    "\\leq",
    "=<",
    "≤",
    ">",
    ">=",
    "\\geq",
    "≥",
    "=>",
    "⇒",
    "\\cup",
    "\\union",
    "∪",
    "\\cap",
    "\\intersect",
    "∩",
    "\\",
    "\\subseteq",
    "⊆",
    "\\o",
    "\\circ",
    "<=>",
    "\\equiv",
    "⇔",
  ].map((name) => [name, 2] as const),
  ["\\in", 2],
  ["∈", 2],
  ["\\notin", 2],
  ["∉", 2],
  ["Seq", 1],
  ["=", 2],
  ["~>", 2],
  ["..", 2],
  ["Append", 2],
  ["SubSeq", 3],
  ["ApaFoldSet", 3],
  ["ApaFoldSeqLeft", 3],
  ["user call representative", 2],
];
const sourceSpellings: Partial<Record<Source["kind"], readonly string[]>> = {
  Always: ["[]", "□"],
  Eventually: ["<>", "◇"],
  And: ["/\\", "\\land", "∧"],
  Or: ["\\/", "\\lor", "∨"],
  Forall: ["\\A", "\\forall", "∀"],
  Exists: ["\\E", "\\exists", "∃"],
  Choose: ["CHOOSE"],
  Enabled: ["ENABLED"],
  Unchanged: ["UNCHANGED"],
  Prime: ["'"],
  BoxAction: ["[A]_v"],
  AngleAction: ["<<A>>_v", "⟨A>>_v"],
  Tuple: ["<<...>>", "⟨...⟩"],
  WeakFairness: ["WF_"],
  StrongFairness: ["SF_"],
  Label: ["::"],
};
const tableRows = (
  space: string,
  nodeKind: string,
  operatorClass: string,
  descriptor: Readonly<Record<string, Field>>,
  sourceArrays = false,
): AnalysisRole[] => {
  const rows: AnalysisRole[] = [];
  const collect = (spec: Field, position: string): void => {
    if (spec === a) return;
    if ("type" in spec)
      rows.push({
        space,
        constructor: nodeKind,
        operatorClass,
        position,
        role: spec.role,
        scope: spec.scope,
        forwarding: spec.forwarding,
      });
    else if ("each" in spec)
      for (const index of sourceArrays ? ["0", "1"] : ["*"])
        collect(spec.each, `${position}[${index}]`);
    else if ("tuple" in spec)
      spec.tuple.forEach((part, index) => {
        collect(part, `${position}[${index}]`);
      });
    else if ("fields" in spec)
      for (const [key, part] of Object.entries(spec.fields))
        collect(part, `${position}${position ? "." : ""}${key}`);
    else
      for (const [variant, part] of Object.entries(spec.variants))
        collect(
          part,
          `${position}${nodeKind === "Except" ? `.${variant}` : ""}`,
        );
  };
  collect({ fields: descriptor }, "");
  if (!rows.length)
    rows.push({
      space,
      constructor: nodeKind,
      operatorClass,
      position: "",
      role: "leaf",
      scope: ["bound", "parameter", "capture"].includes(nodeKind)
        ? "frame"
        : "caller",
      forwarding: { action: "stop", membership: "stop" },
    });
  return rows;
};
const parserRole = (
  nodeKind: string,
  position: string,
  role = "parser-or-context-refusal",
  scope = "syntax",
): AnalysisRole => ({
  space: "parser-only",
  constructor: nodeKind,
  operatorClass: "",
  position,
  role,
  scope,
  forwarding: { action: "stop", membership: "stop" },
});
const makeRoles = (): readonly AnalysisRole[] => {
  const rows: AnalysisRole[] = [];
  const parserKinds = new Set([
    "Decimal",
    "OperatorReference",
    "Lambda",
    "Bracket",
    "Product",
    "Subexpression",
    "UnboundedQuantifier",
  ]);
  for (const kind of Object.keys(sourceDescriptors) as Source["kind"][]) {
    if (parserKinds.has(kind) || kind === "Apply") continue;
    for (const spelling of sourceSpellings[kind] ?? [""])
      rows.push(
        ...tableRows("source", kind, spelling, sourceDescriptors[kind], true),
      );
  }
  for (const [operator, arity] of applyArities)
    rows.push(
      ...tableRows(
        "source",
        "Apply",
        `${operator}/${arity}`,
        {
          ...sourceDescriptors.Apply,
          args: { tuple: applyArguments(operator, arity) },
        },
        true,
      ),
    );
  for (const [nodeKind, descriptor] of Object.entries(expressionDescriptors))
    rows.push(...tableRows("ir-expression", nodeKind, "", descriptor));
  for (const [nodeKind, descriptor] of Object.entries(domainDescriptors))
    rows.push(...tableRows("ir-domain", nodeKind, "", descriptor));
  for (const [nodeKind, descriptor] of Object.entries(stepDescriptors))
    rows.push(...tableRows("ir-step", nodeKind, "", descriptor));
  for (const [nodeKind, descriptor] of Object.entries(temporalDescriptors))
    rows.push(...tableRows("ir-temporal", nodeKind, "", descriptor));
  rows.push(
    ...tableRows("ir-binding", "binding", "", bindingDescriptor),
    ...tableRows("ir-branch", "branch", "", branchDescriptor),
    ...tableRows("ir-definition", "definition", "", definitionDescriptor),
    ...tableRows("ir-definition", "definition", "", actionDefinitionDescriptor),
    ...tableRows("ir-program", "program", "", programDescriptor),
  );
  rows.push(
    ...tableRows("source-wrapper", "Bound", "", { pattern: a, set: source() }),
    ...tableRows("source-wrapper", "CaseArm", "", {
      condition: source(),
      body: source(),
    }),
    ...tableRows("source-wrapper", "RecordField", "", {
      name: a,
      value: source(),
      location: a,
    }),
  );
  rows.push(
    ...tableRows("source-wrapper", "ExceptUpdate", "", {
      path: each(edge("source-except-path")),
      value: source(),
    }),
  );
  rows.push(
    ...tableRows("source-wrapper", "ExceptPath.Index", "", {
      kind: a,
      args: each(source("EXCEPT-key")),
    }),
  );
  rows.push({
    space: "source-wrapper",
    constructor: "ExceptPath.Field",
    operatorClass: "",
    position: "",
    role: "EXCEPT-key",
    scope: "caller",
    forwarding: { action: "stop", membership: "stop" },
  });
  rows.push(
    ...tableRows(
      "source-wrapper",
      "OperatorDefinition",
      "",
      sourceDefinitionDescriptors.Operator,
    ),
    ...tableRows(
      "source-wrapper",
      "FunctionDefinition",
      "",
      sourceDefinitionDescriptors.Function,
    ),
  );
  rows.push(
    parserRole("Decimal", ""),
    parserRole("OperatorReference", ""),
    parserRole("Lambda", "body"),
    parserRole("UnboundedQuantifier", "body"),
    parserRole("Subexpression", "selectors[*].args[*]"),
    parserRole("LetInstance", "instances[*]"),
    parserRole("InstanceSubstitution", "expression"),
    parserRole("Fairness", "enabled", "annotation", "annotation"),
  );
  for (const [nodeKind, field] of [
    ["Bracket", "args"],
    ["Product", "factors"],
  ] as const)
    for (const index of [0, 1])
      rows.push(parserRole(nodeKind, `${field}[${index}]`));
  const key = (row: AnalysisRole) =>
    `${row.space}\0${row.constructor}\0${row.operatorClass}\0${row.position}`;
  return rows.sort((left, right) =>
    key(left) < key(right) ? -1 : key(left) > key(right) ? 1 : 0,
  );
};
export const analysisRoles: readonly AnalysisRole[] = makeRoles();
