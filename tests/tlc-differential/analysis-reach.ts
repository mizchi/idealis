import assert from "node:assert/strict";
import { isRecord } from "../../packages/idealis/src/trace.js";
import {
  type AnalysisCase,
  analysisLimits,
  sourceObjects,
  sourceReach,
} from "./analysis-parity-corpus.js";

type Node = Record<string, unknown>;
const record = (value: unknown, label: string): Node => {
  assert.ok(isRecord(value), label);
  return value;
};
const items = (value: unknown, label: string): readonly unknown[] => {
  assert.ok(Array.isArray(value), label);
  return value;
};
const literal = (value: unknown, expected: string | boolean) =>
  isRecord(value) &&
  value.op === "literal" &&
  isRecord(value.value) &&
  value.value.type === (typeof expected === "boolean" ? "bool" : "int") &&
  value.value.value === expected;
const number = (value: unknown, expected: number) =>
  isRecord(value) && value.kind === "Number" && value.value === expected;
const at = (value: unknown, path: string): unknown => {
  let current = value;
  for (const part of path
    .replaceAll("[", ".")
    .replaceAll("]", "")
    .split(".")
    .filter(Boolean)) {
    current = Array.isArray(current)
      ? current[Number(part)]
      : isRecord(current)
        ? current[part]
        : undefined;
  }
  return current;
};
const sourceCall = (value: Node, name: string) =>
  value.kind === "Apply" && value.operator === name;
const sourceProbe = (entry: AnalysisCase, value: Node): boolean => {
  if (entry.probe === "binder") {
    if (sourceCall(value, "Hit")) return true;
    return (
      sourceCall(value, "=") &&
      isRecord(at(value, "args[0]")) &&
      at(value, "args[0].kind") === "Forall" &&
      sourceObjects(value).some((node) => sourceCall(node, "Hit"))
    );
  }
  if (entry.probe === "target")
    return (
      sourceCall(value, "Can") ||
      (entry.control &&
        value.kind === "Enabled" &&
        at(value, "expression.kind") === "Unchanged")
    );
  if (entry.probe === "membership")
    return (
      sourceCall(value, "Member") ||
      sourceCall(value, "M") ||
      (sourceCall(value, "\\in") && at(value, "args[1].name") === "p")
    );
  return (
    sourceCall(value, "=") &&
    items(value.args, "old comparison arguments").some((arg) =>
      entry.control ? number(arg, 9) : isRecord(arg) && arg.kind === "ExceptAt",
    )
  );
};
const probeDepth = (entry: AnalysisCase, value: unknown): number => {
  if (Array.isArray(value))
    return Math.max(0, ...value.map((child) => probeDepth(entry, child)));
  if (!isRecord(value)) return 0;
  if (sourceProbe(entry, value)) return 1;
  const children = Object.entries(value)
    .filter(([key]) => key !== "location")
    .map(([, child]) => probeDepth(entry, child));
  const depth = Math.max(0, ...children);
  return depth === 0
    ? 0
    : depth +
        (typeof value.kind === "string" &&
        !["Index", "Field", "Name"].includes(value.kind)
          ? 1
          : 0);
};
const binderCount = (value: Node): number => {
  if (
    ["Forall", "Exists", "FunctionConstruction", "SetMap"].includes(
      String(value.kind),
    )
  )
    return items(value.bounds, "source binder domains").length;
  return value.kind === "Choose" || value.kind === "SetFilter" ? 1 : 0;
};
const maxBindings = (value: unknown, active = 0): number => {
  if (Array.isArray(value))
    return Math.max(
      active,
      ...value.map((child) => maxBindings(child, active)),
    );
  if (!isRecord(value)) return active;
  const next = active + binderCount(value);
  const values = Object.entries(value)
    .filter(([key]) => key !== "location")
    .map(([key, child]) =>
      maxBindings(child, key === "definitions" ? 0 : next),
    );
  return Math.max(next, ...values);
};
const positionAnchor = (entry: AnalysisCase, inspect: Node): Node => {
  const expected = entry.position.constructor;
  const candidates = sourceObjects(inspect.body).filter(
    (node) =>
      node.kind === expected &&
      (expected !== "Apply" ||
        node.operator === entry.position.spelling ||
        (Array.isArray(node.args) &&
          node.args.length === entry.position.arity)),
  );
  const path = entry.position.position.replaceAll(".Index", "");
  const found = candidates.find((node) =>
    sourceObjects(at(node, path)).some((child) => sourceProbe(entry, child)),
  );
  if (found) return found;
  if (
    entry.position.spelling.startsWith("ApaFold") &&
    entry.position.position === "args[0]"
  ) {
    const fold = candidates.find((node) =>
      String(node.operator).startsWith("ApaFold"),
    );
    assert.ok(fold, "fold operator reference source adapter");
    return fold;
  }
  assert.fail(
    `constructor anchor must contain the selected source probe: ${entry.id}`,
  );
};

export const assertSourceReach = (entry: AnalysisCase): void => {
  const reached = sourceReach(entry);
  assert.equal(
    reached.sentinel.length,
    1,
    `one source sentinel at ${entry.sourcePath}`,
  );
  const all = sourceObjects(reached.module);
  const definitionCount =
    reached.module.definitions.length +
    all
      .filter((node) => node.kind === "Let")
      .reduce(
        (count, node) =>
          count + items(node.definitions, "LET declarations").length,
        0,
      );
  assert.ok(
    definitionCount <= analysisLimits.definitions,
    `${definitionCount} recursive definitions exceed ${analysisLimits.definitions}`,
  );
  for (const definition of reached.module.definitions)
    assert.ok(
      maxBindings(definition.body) <= analysisLimits.binders,
      `binder slots per declaration: ${entry.id}`,
    );
  const anchor = positionAnchor(entry, reached.inspect);
  let depth = probeDepth(entry, anchor);
  if (
    entry.position.spelling.startsWith("ApaFold") &&
    entry.position.position === "args[0]"
  ) {
    const fold = all.find((node) => node.name === "Fold" && "body" in node);
    assert.ok(fold);
    assert.equal(items(fold.params, "fold formal arguments").length, 2);
    depth = 1 + probeDepth(entry, fold.body);
  }
  assert.ok(
    depth > 0 && depth <= analysisLimits.recipeDepth,
    `recipe depth ${depth} exceeds ${analysisLimits.recipeDepth}: ${entry.id}`,
  );
  if (entry.probe === "binder") {
    const call = reached.sentinel[0];
    assert.ok(call && Array.isArray(call.args));
    const actual = call.args[0];
    if (entry.control)
      assert.ok(number(actual, 9), "binder control must pass literal9");
    else {
      assert.ok(
        isRecord(actual) &&
          actual.kind === "Identifier" &&
          ["p", "n"].includes(String(actual.name)),
        "binder probe must pass caller binding",
      );
      const outer = all.filter(
        (node) =>
          node.kind === "Forall" &&
          sourceObjects(node.body).some(
            (node) => sourceCall(node, "Hit") || sourceCall(node, "Inspect"),
          ),
      );
      assert.ok(
        outer.some((node) =>
          sourceObjects(node.bounds).some(
            (node) =>
              number(node, 9) ||
              (node.kind === "Identifier" && node.name === "p"),
          ),
        ),
        "caller binder singleton9 or caller formal domain",
      );
    }
    const hit = all.find((node) => node.name === "Hit" && "body" in node);
    assert.ok(hit);
    if (entry.position.spelling === "~>") {
      assert.equal(at(hit, "body.kind"), "Apply");
      assert.equal(at(hit, "body.operator"), "=");
      assert.equal(at(hit, "body.args[0].name"), "v");
      assert.ok(number(at(hit, "body.args[1]"), 9));
    } else {
      assert.equal(at(hit, "body.kind"), "Forall");
      assert.ok(
        number(at(hit, "body.bounds[0].set.elements[0]"), 4),
        "callee shadow binder must differ from caller9",
      );
    }
  }
  if (entry.probe === "membership") {
    const parameterCalls = all.filter((node) =>
      ["Inspect", "Outer", "Member", "M"].some((name) =>
        sourceCall(node, name),
      ),
    );
    assert.ok(
      parameterCalls.some((call) =>
        entry.control
          ? at(call, "args[0].kind") === "SetEnumeration" &&
            number(at(call, "args[0].elements[0]"), 0) &&
            items(at(call, "args[0].elements"), "finite control singleton")
              .length === 1
          : at(call, "args[0].kind") === "Identifier" &&
            at(call, "args[0].name") === "Nat",
      ),
      "Nat probe and singleton finite control must differ at the caller actual",
    );
  }
};

const graph = (program: unknown) => {
  const root = record(program, "compiled v2 program");
  assert.equal(root.version, 2);
  const definitions = items(root.definitions, "v2 definitions").map((value) =>
    record(value, "definition"),
  );
  const byId = new Map(
    definitions.map((definition) => [definition.id, definition]),
  );
  assert.equal(byId.size, definitions.length, "definition IDs remain unique");
  const safe = [
    "invariants",
    "initialProperties",
    "actionProperties",
    "properties",
  ]
    .flatMap((field) => (Array.isArray(root[field]) ? root[field] : []))
    .find((value) => isRecord(value) && value.name === "Safe");
  assert.ok(isRecord(safe), "named Safe predicate or property");
  const body = safe.expression ?? safe.formula;
  assert.ok(isRecord(body), "Safe root expression/formula");
  const nodes = (value: unknown): readonly Node[] => {
    const result: Node[] = [];
    const pending = [value];
    const seen = new Set<unknown>();
    while (pending.length) {
      const current = pending.pop();
      if (Array.isArray(current)) pending.push(...current);
      else if (isRecord(current) && !seen.has(current)) {
        seen.add(current);
        result.push(current);
        pending.push(...Object.values(current));
        if (current.op === "call" || current.kind === "call") {
          const definition = byId.get(current.definition);
          assert.ok(definition, "call retains an existing definition ID");
          assert.equal(
            items(current.arguments, "call arguments").length,
            definition.parameterCount,
          );
          assert.equal(
            items(current.captures, "call captures").length,
            definition.captureCount,
          );
          pending.push(definition.body);
        }
        if (current.op === "frame" || current.kind === "frame") {
          assert.equal(
            items(current.arguments, "frame arguments").length,
            current.parameterCount,
          );
          assert.equal(
            items(current.captures, "frame captures").length,
            current.captureCount,
          );
          assert.ok(
            typeof current.localScope === "number" &&
              current.localScope >= 0 &&
              current.localScope <= 40,
            "frame localScope",
          );
        }
      }
    }
    return result;
  };
  return { root, safe, body, byId, nodes };
};
const hitBody = (body: unknown): boolean => {
  const comparison = isRecord(body) && body.op === "forall" ? body.body : body;
  const validComparison =
    isRecord(comparison) &&
    comparison.op === "eq" &&
    at(comparison, "left.op") === "parameter" &&
    at(comparison, "left.slot") === 0 &&
    literal(comparison.right, "9");
  return (
    validComparison &&
    isRecord(body) &&
    (body.op === "eq" ||
      (body.op === "forall" &&
        literal(at(body, "bounds[0].domain.values[0]"), "4")))
  );
};
const enabledTarget = (node: Node): boolean =>
  node.op === "enabled" &&
  Array.isArray(node.branches) &&
  node.branches.length === 1 &&
  at(node, "branches[0].steps[0].kind") === "assign" &&
  at(node, "branches[0].steps[0].slot") === 0 &&
  at(node, "branches[0].steps[0].value.op") === "read" &&
  at(node, "branches[0].steps[0].value.slot") === 0 &&
  at(node, "branches[0].steps[0].value.next") === false;
const probeIn = (
  entry: AnalysisCase,
  value: unknown,
  view: ReturnType<typeof graph>,
): boolean =>
  view.nodes(value).some((node) => {
    if (entry.probe === "binder") {
      if (node.op !== "call") return false;
      const definition = view.byId.get(node.definition);
      if (!definition || !hitBody(definition.body)) return false;
      assert.equal(definition.parameterCount, 1);
      assert.equal(definition.captureCount, 0);
      const argument = at(node, "arguments[0]");
      return entry.control
        ? literal(argument, "9")
        : isRecord(argument) &&
            ["bound", "parameter", "capture"].includes(String(argument.op)) &&
            argument.slot === 0;
    }
    if (entry.probe === "target") return enabledTarget(node);
    if (entry.probe === "membership")
      return (
        ["function-in", "in"].includes(String(node.op)) &&
        (literal(node.left ?? node.value, "0") ||
          (at(node.left ?? node.value, "op") === "read" &&
            at(node.left ?? node.value, "slot") === 0)) &&
        ((isRecord(node.right) &&
          ["parameter", "capture"].includes(String(node.right.op))) ||
          isRecord(node.domain))
      );
    return entry.control
      ? node.op === "eq" && literal(node.left, "9") && literal(node.right, "9")
      : node.op === "old";
  });
const unaryOps: Record<string, string> = {
  "~": "not",
  "\\lnot": "not",
  "\\neg": "not",
  "¬": "not",
  DOMAIN: "domain",
  Cardinality: "cardinality",
  Len: "len",
  Head: "head",
  Tail: "tail",
  SUBSET: "powerset",
  UNION: "union",
};
const canonical: Record<string, string> = {
  "∈": "\\in",
  "∉": "\\notin",
  "\\equiv": "<=>",
  "⇔": "<=>",
};
const projection = (
  entry: AnalysisCase,
  node: Node,
): readonly unknown[] | null => {
  const kind = entry.position.constructor;
  const position = entry.position.position;
  const index = Number(position.match(/\[(\d+)\]/)?.[1]);
  const op = node.op ?? node.kind;
  if (kind === "And" || kind === "Or")
    return op === (kind === "And" ? "and" : "or")
      ? [node[index === 0 ? "left" : "right"]]
      : null;
  if (kind === "Prime") return op === "prime" ? [node.argument] : null;
  if (kind === "Unchanged") return op === "eq" ? [node.left] : null;
  if (kind === "Enabled") return op === "enabled" ? [node.branches] : null;
  if (kind === "Eventually") return op === "eventually" ? [node.formula] : null;
  if (kind === "If") return op === "if" ? [node[position]] : null;
  if (kind === "Case") {
    if (op !== "if" || at(node, "alternative.op") !== "if") return null;
    if (position === "other") return [at(node, "alternative.alternative")];
    return [
      at(
        node,
        `${index === 0 ? "" : "alternative."}${position.endsWith("condition") ? "condition" : "consequent"}`,
      ),
    ];
  }
  if (
    [
      "Forall",
      "Exists",
      "FunctionConstruction",
      "SetMap",
      "SetFilter",
    ].includes(kind)
  ) {
    if (
      op !==
      (
        {
          Forall: "forall",
          Exists: "exists",
          FunctionConstruction: "function",
          SetMap: "map-set",
          SetFilter: "filter",
        } as Record<string, string>
      )[kind]
    )
      return null;
    return [
      position.includes(".set")
        ? at(node, `bounds[${Number.isNaN(index) ? 0 : index}].domain`)
        : node.body,
    ];
  }
  if (kind === "Choose")
    return op === "choose"
      ? [position === "set" ? at(node, "binding.domain") : node.body]
      : null;
  if (kind === "SetEnumeration" || kind === "Tuple")
    return op === (kind === "Tuple" ? "tuple" : "set")
      ? [at(node, `values[${index}]`)]
      : null;
  if (kind === "FunctionSet")
    return op === "function-set"
      ? [node[position === "domain" ? "left" : "right"]]
      : null;
  if (kind === "Record" || kind === "RecordSet")
    return op === (kind === "Record" ? "record" : "record-set")
      ? [at(node, `fields[${index}][1]`)]
      : null;
  if (kind === "Except") {
    if (op !== "update") return null;
    if (position === "function") return [node.target];
    const indices = [...position.matchAll(/\[(\d+)\]/g)].map((match) =>
      Number(match[1]),
    );
    return [
      position.endsWith(".value")
        ? at(node, `updates[${indices[0]}].value`)
        : at(
            node,
            `updates[${indices[0]}].path[${indices[1]}].values[${indices[2]}]`,
          ),
    ];
  }
  if (kind === "FunctionApplication")
    return op === "at"
      ? [
          position === "function"
            ? node.target
            : at(node, `key.values[${index}]`),
        ]
      : null;
  if (kind === "FieldAccess") return op === "at" ? [node.target] : null;
  if (kind === "BoxAction" || kind === "AngleAction")
    return op === (kind === "BoxAction" ? "or" : "and")
      ? [
          position === "action"
            ? node.left
            : at(
                node,
                kind === "BoxAction" ? "right.left" : "right.argument.left",
              ),
        ]
      : null;
  if (kind === "WeakFairness" || kind === "StrongFairness")
    return op === "enabled" && "subscript" in node
      ? [position === "subscript" ? node.subscript : node.branches]
      : null;
  if (kind === "Apply") {
    const spelling =
      canonical[entry.position.spelling] ?? entry.position.spelling;
    if (spelling.startsWith("ApaFold"))
      return op === "fold"
        ? [node[index === 0 ? "body" : index === 1 ? "initial" : "collection"]]
        : null;
    if (spelling === "user call representative" || spelling === "user call")
      return op === "call" &&
        Array.isArray(node.arguments) &&
        node.arguments.length === 2
        ? [node.arguments[index]]
        : null;
    if (spelling === "Seq") return op === "sequence-in" ? [node.right] : null;
    if (spelling === "\\in")
      return op === "in" ? [index === 0 ? node.value : node.domain] : null;
    if (spelling === "\\notin")
      return op === "not" && at(node, "argument.op") === "in"
        ? [at(node, index === 0 ? "argument.value" : "argument.domain")]
        : null;
    if (spelling === "SubSeq")
      return op === "subseq"
        ? [node[["sequence", "from", "to"][index] ?? ""]]
        : null;
    if (spelling === "..")
      return op === "map-set" && at(node, "bounds[0].domain.kind") === "range"
        ? [at(node, `bounds[0].domain.${index === 0 ? "lower" : "upper"}`)]
        : null;
    if (spelling === "<=>")
      return op === "eq" &&
        at(node, "left.op") === "not" &&
        at(node, "right.op") === "not"
        ? [at(node, `${index === 0 ? "left" : "right"}.argument`)]
        : null;
    if (entry.position.arity === 1)
      return op === (spelling === "-" ? "neg" : unaryOps[spelling])
        ? [node.argument]
        : null;
    return op === entry.expectedOp
      ? [node[index === 0 ? "left" : "right"]]
      : null;
  }
  if (kind === "Let" || kind === "Label") return [node];
  return null;
};
const formulaBody = (value: unknown, view?: ReturnType<typeof graph>): Node => {
  let node = record(value, "temporal formula");
  const visited = new Set<unknown>();
  while (node.kind === "frame" || node.kind === "call") {
    if (node.kind === "frame")
      node = record(node.formula, "temporal frame body");
    else {
      const definition = view?.byId.get(node.definition);
      assert.ok(definition && definition.bodyKind === "temporal");
      assert.ok(
        !visited.has(definition.id),
        "acyclic temporal constructor projection",
      );
      visited.add(definition.id);
      node = record(definition.body, "temporal call body");
    }
  }
  return node;
};
const assertLeadsTo = (
  entry: AnalysisCase,
  view: ReturnType<typeof graph>,
): void => {
  const formula = formulaBody(view.body, view);
  assert.equal(formula.kind, "always", "leads-to outer always");
  const implication = record(formula.formula, "leads-to implication");
  assert.equal(implication.kind, "or");
  const left = record(implication.left, "leads-to antecedent negation");
  const right = record(implication.right, "leads-to consequent eventuality");
  assert.equal(right.kind, "eventually");
  let antecedent: unknown;
  if (left.kind === "not") antecedent = left.formula;
  else {
    assert.equal(left.kind, "atom");
    assert.equal(at(left, "expression.op"), "not");
    antecedent = { kind: "atom", expression: at(left, "expression.argument") };
  }
  const consequent = right.formula;
  const index = Number(entry.position.position.match(/\[(\d+)\]/)?.[1]);
  const selected = index === 0 ? antecedent : consequent;
  const opposite = formulaBody(index === 0 ? consequent : antecedent, view);
  assert.equal(
    opposite.kind,
    "atom",
    "opposite leads-to side remains TRUE atom",
  );
  assert.ok(literal(opposite.expression, true));
  assert.ok(
    probeIn(entry, selected, view),
    `probe must remain on leads-to side ${index}`,
  );
  assert.equal(
    probeIn(entry, opposite, view),
    false,
    "leads-to probe must not migrate to opposite side",
  );
};

export const assertIrReach = (entry: AnalysisCase, program: unknown): void => {
  const view = graph(program);
  const nodes = view.nodes(view.body);
  if (
    entry.probe === "old" &&
    entry.refusal === null &&
    !(
      entry.position.constructor === "Except" &&
      entry.position.position.endsWith(".value")
    )
  ) {
    assert.ok(
      !nodes.some((node) => node.op === "old"),
      "unused LET body must not introduce an old read into the reachable IR",
    );
    return;
  }
  if (entry.position.spelling === "~>") {
    assertLeadsTo(entry, view);
  } else if (entry.position.constructor === "Always") {
    assert.ok(
      items(view.root.invariants, "always invariant classification").some(
        (predicate) => isRecord(predicate) && predicate.name === "Safe",
      ),
    );
    assert.ok(probeIn(entry, view.body, view));
  } else {
    const candidates = nodes
      .map((node) => ({ node, values: projection(entry, node) }))
      .filter((candidate) => candidate.values !== null);
    assert.ok(
      candidates.length > 0,
      `independent IR constructor projection ${entry.position.constructor}/${entry.position.position}`,
    );
    assert.ok(
      candidates.some((candidate) =>
        candidate.values?.some(
          (value) => value !== undefined && probeIn(entry, value, view),
        ),
      ),
      `probe must occupy its lowered IR position ${entry.position.constructor}/${entry.position.position}`,
    );
  }
  if (entry.probe === "membership") {
    const expectedActual = nodes.some((node) =>
      entry.control
        ? node.op === "set" &&
          Array.isArray(node.values) &&
          node.values.length === 1 &&
          literal(node.values[0], "0")
        : node.op === "map-set" &&
          at(node, "bounds[0].domain.kind") === "range" &&
          literal(at(node, "bounds[0].domain.lower"), "0") &&
          literal(at(node, "bounds[0].domain.upper"), "2147483647") &&
          at(node, "body.op") === "bound",
    );
    assert.ok(
      expectedActual,
      "caller membership actual retains finite singleton or Nat range descriptor",
    );
  }
};
