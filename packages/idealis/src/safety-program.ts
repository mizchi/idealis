import type { SafetyProgram } from "./contracts.js";
import { normalizePortableValue, PortableCoreError } from "./portable-value.js";
import { isRecord } from "./trace.js";

export const WIRE_INDEX_MAX = 4_294_967_295;

const fields = (
  input: unknown,
  required: readonly string[],
  optional: readonly string[] = [],
) => {
  if (
    !isRecord(input) ||
    required.some((key) => !Object.hasOwn(input, key)) ||
    Object.keys(input).some(
      (key) => !required.includes(key) && !optional.includes(key),
    )
  )
    throw new PortableCoreError("protocol", "Invalid compiled program fields");
  return input;
};
const array = (input: unknown): readonly unknown[] => {
  if (!Array.isArray(input))
    throw new PortableCoreError("protocol", "Expected compiled program array");
  count(input.length);
  return input;
};
const text = (input: unknown): void => {
  if (typeof input !== "string" || !input.length)
    throw new PortableCoreError(
      "protocol",
      "Expected compiled program identifier",
    );
};
const count = (input: unknown): void => {
  if (
    typeof input !== "number" ||
    !Number.isSafeInteger(input) ||
    input < 0 ||
    input > WIRE_INDEX_MAX
  )
    throw new PortableCoreError("protocol", "Invalid compiled program count");
};
const domain = (input: unknown, depth: number, version: 1 | 2): void => {
  const value = fields(input, ["kind"], ["values", "lower", "upper", "value"]);
  switch (value.kind) {
    case "set":
      fields(value, ["kind", "values"]);
      array(value.values).forEach((item) => {
        expression(item, depth, version);
      });
      return;
    case "range":
      fields(value, ["kind", "lower", "upper"]);
      expression(value.lower, depth, version);
      expression(value.upper, depth, version);
      return;
    case "value":
      fields(value, ["kind", "value"]);
      expression(value.value, depth, version);
      return;
    default:
      throw new PortableCoreError("protocol", "Invalid compiled domain");
  }
};
const binding = (input: unknown, depth: number, version: 1 | 2): void => {
  const value = fields(input, ["slot", "domain"], ["tupleSize"]);
  count(value.slot);
  domain(value.domain, depth, version);
  if (value.tupleSize !== undefined) count(value.tupleSize);
};
const frame = (
  input: Record<string, unknown>,
  tag: "op" | "kind",
  body: "body" | "branches" | "formula",
  depth: number,
  version: 1 | 2,
): void => {
  if (version !== 2 || depth > 128)
    throw new PortableCoreError("protocol", "Invalid compiled frame");
  fields(input, [
    tag,
    "arguments",
    "captures",
    "parameterCount",
    "captureCount",
    "localScope",
    body,
  ]);
  count(input.parameterCount);
  count(input.captureCount);
  count(input.localScope);
  if (
    input.parameterCount !== array(input.arguments).length ||
    input.captureCount !== array(input.captures).length ||
    Number(input.localScope) > 40
  )
    throw new PortableCoreError("protocol", "Invalid compiled frame signature");
  for (const item of [...array(input.arguments), ...array(input.captures)])
    expression(item, depth + 1, version);
};
const branches = (input: unknown, depth: number, version: 1 | 2): void => {
  for (const item of array(input)) {
    const branch = fields(item, ["name", "steps"]);
    text(branch.name);
    for (const input of array(branch.steps)) {
      if (!isRecord(input))
        throw new PortableCoreError("protocol", "Invalid compiled step");
      switch (input.kind) {
        case "call":
          if (version !== 2)
            throw new PortableCoreError(
              "protocol",
              "Action calls require safety IR v2",
            );
          fields(input, ["kind", "definition", "arguments", "captures"]);
          count(input.definition);
          for (const value of [
            ...array(input.arguments),
            ...array(input.captures),
          ])
            expression(value, depth + 1, version);
          break;
        case "frame":
          frame(input, "kind", "branches", depth, version);
          branches(input.branches, depth + 1, version);
          break;
        case "bind": {
          const step = fields(input, ["kind", "slot", "domain"], ["tupleSize"]);
          binding(
            {
              slot: step.slot,
              domain: step.domain,
              ...(step.tupleSize === undefined
                ? {}
                : { tupleSize: step.tupleSize }),
            },
            depth,
            version,
          );
          break;
        }
        case "choose": {
          const step = fields(input, ["kind", "slot", "domain"]);
          count(step.slot);
          domain(step.domain, depth, version);
          break;
        }
        case "assign": {
          const step = fields(input, ["kind", "slot", "value"]);
          count(step.slot);
          expression(step.value, depth, version);
          break;
        }
        case "guard": {
          const step = fields(input, ["kind", "expression"]);
          expression(step.expression, depth, version);
          break;
        }
        default:
          throw new PortableCoreError("protocol", "Invalid compiled step kind");
      }
    }
  }
};
const expression = (input: unknown, depth: number, version: 1 | 2): void => {
  if (!isRecord(input) || depth > 128)
    throw new PortableCoreError("protocol", "Invalid compiled expression");
  const child = depth + 1;
  const unary = [
    "prime",
    "not",
    "neg",
    "domain",
    "variant-tag",
    "cardinality",
    "sequence-set",
    "len",
    "head",
    "tail",
    "powerset",
    "union",
  ];
  const binary = [
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
  ];
  if (typeof input.op === "string" && unary.includes(input.op)) {
    const value = fields(input, ["op", "argument"]);
    expression(value.argument, child, version);
    return;
  }
  if (typeof input.op === "string" && binary.includes(input.op)) {
    const value = fields(input, ["op", "left", "right"]);
    expression(value.left, child, version);
    expression(value.right, child, version);
    return;
  }
  switch (input.op) {
    case "frame":
      frame(input, "op", "body", depth, version);
      expression(input.body, child, version);
      return;
    case "call":
      if (version !== 2)
        throw new PortableCoreError("protocol", "Unknown compiled expression");
      fields(input, ["op", "definition", "arguments", "captures"]);
      count(input.definition);
      for (const item of [...array(input.arguments), ...array(input.captures)])
        expression(item, child, version);
      return;
    case "parameter":
    case "capture":
      if (version !== 2)
        throw new PortableCoreError("protocol", "Unknown compiled expression");
      fields(input, ["op", "slot"]);
      count(input.slot);
      return;
    case "unmatched-case":
      fields(input, ["op", "location"]);
      text(input.location);
      return;
    case "old":
      fields(input, ["op"]);
      return;
    case "data":
      fields(input, ["op", "value"]);
      normalizePortableValue(input.value);
      return;
    case "literal": {
      const value = fields(input, ["op", "value"]);
      const scalar = normalizePortableValue(value.value);
      if (scalar.type !== "int" && scalar.type !== "bool")
        throw new PortableCoreError("protocol", "Invalid compiled scalar");
      return;
    }
    case "bound":
      fields(input, ["op", "slot"]);
      count(input.slot);
      return;
    case "read":
      fields(input, ["op", "slot", "next"]);
      count(input.slot);
      if (typeof input.next !== "boolean")
        throw new PortableCoreError("protocol", "Invalid compiled read");
      return;
    case "set":
    case "tuple":
      fields(input, ["op", "values"]);
      array(input.values).forEach((item) => {
        expression(item, child, version);
      });
      return;
    case "record":
    case "record-set":
      fields(input, ["op", "fields"]);
      for (const item of array(input.fields)) {
        const pair = array(item);
        if (pair.length !== 2 || typeof pair[0] !== "string")
          throw new PortableCoreError("protocol", "Invalid compiled field");
        expression(pair[1], child, version);
      }
      return;
    case "at":
      fields(input, ["op", "target", "key"]);
      expression(input.target, child, version);
      expression(input.key, child, version);
      return;
    case "update":
      fields(input, ["op", "target", "updates"]);
      expression(input.target, child, version);
      for (const item of array(input.updates)) {
        const update = fields(item, ["path", "value"]);
        array(update.path).forEach((part) => {
          expression(part, child, version);
        });
        expression(update.value, child, version);
      }
      return;
    case "forall":
    case "exists":
    case "function":
    case "filter":
    case "map-set":
      fields(input, ["op", "bounds", "body"]);
      array(input.bounds).forEach((item) => {
        binding(item, child, version);
      });
      expression(input.body, child, version);
      return;
    case "fold":
      fields(input, [
        "op",
        "accumulator",
        "initial",
        "collection",
        "order",
        "body",
      ]);
      count(input.accumulator);
      if (input.order !== "set-members" && input.order !== "sequence-left")
        throw new PortableCoreError("protocol", "Invalid compiled fold order");
      expression(input.initial, child, version);
      expression(input.collection, child, version);
      expression(input.body, child, version);
      return;
    case "choose":
      fields(input, ["op", "binding", "body"]);
      binding(input.binding, child, version);
      expression(input.body, child, version);
      return;
    case "select-seq":
    case "sort-seq":
      fields(input, ["op", "slot", "sequence", "body"]);
      count(input.slot);
      expression(input.sequence, child, version);
      expression(input.body, child, version);
      return;
    case "assert":
      fields(input, ["op", "condition", "message"]);
      expression(input.condition, child, version);
      expression(input.message, child, version);
      return;
    case "if":
      fields(input, ["op", "condition", "consequent", "alternative"]);
      expression(input.condition, child, version);
      expression(input.consequent, child, version);
      expression(input.alternative, child, version);
      return;
    case "subseq":
      fields(input, ["op", "sequence", "from", "to"]);
      expression(input.sequence, child, version);
      expression(input.from, child, version);
      expression(input.to, child, version);
      return;
    case "in":
      fields(input, ["op", "value", "domain"]);
      expression(input.value, child, version);
      domain(input.domain, child, version);
      return;
    case "enabled":
      fields(input, ["op", "branches"], ["subscript"]);
      branches(input.branches, child, version);
      if (input.subscript === null && version === 1) delete input.subscript;
      if (input.subscript !== undefined && input.subscript !== null)
        expression(input.subscript, child, version);
      return;
    default:
      throw new PortableCoreError("protocol", "Unknown compiled expression");
  }
};
const formula = (input: unknown, version: 1 | 2, depth = 0): void => {
  if (!isRecord(input) || depth > 128)
    throw new PortableCoreError(
      "protocol",
      "Invalid compiled temporal formula",
    );
  switch (input.kind) {
    case "call":
      if (version !== 2)
        throw new PortableCoreError(
          "protocol",
          "Temporal calls require safety IR v2",
        );
      fields(input, ["kind", "definition", "arguments", "captures"]);
      count(input.definition);
      for (const item of [...array(input.arguments), ...array(input.captures)])
        expression(item, 0, version);
      return;
    case "frame":
      frame(input, "kind", "formula", depth, version);
      formula(input.formula, version, depth + 1);
      return;
    case "atom":
      fields(input, ["kind", "expression"]);
      expression(input.expression, 0, version);
      return;
    case "forall":
    case "exists":
      fields(input, ["kind", "bounds", "formula"]);
      array(input.bounds).forEach((item) => {
        binding(item, 0, version);
      });
      formula(input.formula, version, depth + 1);
      return;
    case "not":
    case "always":
    case "eventually":
      fields(input, ["kind", "formula"]);
      formula(input.formula, version, depth + 1);
      return;
    case "and":
    case "or":
      fields(input, ["kind", "left", "right"]);
      formula(input.left, version, depth + 1);
      formula(input.right, version, depth + 1);
      return;
    default:
      throw new PortableCoreError(
        "protocol",
        "Unknown compiled temporal formula",
      );
  }
};
export const decodeSafetyProgram = (input: unknown): SafetyProgram => {
  const cloned: unknown = structuredClone(input);
  if (!isRecord(cloned) || (cloned.version !== 1 && cloned.version !== 2))
    throw new PortableCoreError("protocol", "Invalid compiled program version");
  const version = cloned.version;
  const value = fields(
    cloned,
    [
      "format",
      "version",
      "name",
      "variables",
      "initial",
      "next",
      "invariants",
      "assumptions",
      "checkDeadlock",
      ...(version === 2 ? ["definitions"] : []),
    ],
    [
      "symmetry",
      "initialProperties",
      "actionProperties",
      "properties",
      "fairness",
      "constraints",
      "actionConstraints",
    ],
  );
  if (
    value.format !== "modelt.safety-program" ||
    typeof value.checkDeadlock !== "boolean"
  )
    throw new PortableCoreError("protocol", "Invalid compiled program version");
  if (version === 2)
    array(value.definitions).forEach((input, id) => {
      const definition = fields(
        input,
        [
          "id",
          "parameterCount",
          "captureCount",
          "bodyKind",
          "localScope",
          "body",
        ],
        ["recursive"],
      );
      count(definition.id);
      count(definition.parameterCount);
      count(definition.captureCount);
      count(definition.localScope);
      if (
        definition.id !== id ||
        (definition.recursive !== undefined &&
          typeof definition.recursive !== "boolean") ||
        (definition.bodyKind !== "expression" &&
          definition.bodyKind !== "action" &&
          definition.bodyKind !== "temporal") ||
        (definition.localScope as number) > 40
      )
        throw new PortableCoreError("protocol", "Invalid compiled definition");
      if (definition.bodyKind === "action")
        branches(definition.body, 0, version);
      else if (definition.bodyKind === "temporal")
        formula(definition.body, version);
      else expression(definition.body, 0, version);
    });
  text(value.name);
  if (value.symmetry !== undefined) {
    const symmetry = fields(value.symmetry, ["modelValues"]);
    const members = array(symmetry.modelValues);
    if (
      members.length < 2 ||
      members.length > 64 ||
      new Set(members).size !== members.length ||
      members.some(
        (member) =>
          typeof member !== "string" ||
          !/^[A-Za-z_][A-Za-z_0-9]*$/.test(member) ||
          member[1] === "_",
      )
    )
      throw new PortableCoreError(
        "protocol",
        "Invalid compiled symmetry group",
      );
  }
  const variables = array(value.variables);
  variables.forEach(text);
  if (!variables.length || new Set(variables).size !== variables.length)
    throw new PortableCoreError("protocol", "Invalid compiled variables");
  branches(value.initial, 0, version);
  branches(value.next, 0, version);
  for (const key of [
    "invariants",
    "assumptions",
    "initialProperties",
    "actionProperties",
    "constraints",
    "actionConstraints",
  ]) {
    if (value[key] === undefined) continue;
    for (const item of array(value[key])) {
      const predicate = fields(item, ["name", "expression"]);
      text(predicate.name);
      expression(predicate.expression, 0, version);
    }
  }
  if (value.properties !== undefined)
    for (const item of array(value.properties)) {
      const property = fields(item, ["name", "formula"]);
      text(property.name);
      formula(property.formula, version);
    }
  if (value.fairness !== undefined)
    array(value.fairness).forEach((item) => {
      formula(item, version);
    });
  return value as unknown as SafetyProgram;
};
