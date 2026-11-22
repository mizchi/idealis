import {
  type ConfigValue,
  type Module,
  parseConfig,
  parseModule,
} from "tla-ts";
import type {
  SafetyActionDefinition,
  SafetyBinding as SafetyBindingBase,
  SafetyBranch as SafetyBranchBase,
  SafetyDomain as SafetyDomainBase,
  SafetyExpressionV2 as SafetyExpression,
  SafetyExpressionDefinition,
  SafetyPredicate as SafetyPredicateBase,
  SafetyProgram,
  SafetyStep as SafetyStepBase,
  TemporalFormula as TemporalFormulaBase,
  TlaSourceOptions,
} from "./contracts.js";
import { PortableCoreError } from "./portable-value.js";
import {
  type AnalysisNode,
  analysisChildren,
  analysisView,
  mapAnalysisChildren,
} from "./tla-safety-analysis.js";

const targetExpansionLimit = 524_288;

type SafetyBinding = SafetyBindingBase<2>;
type SafetyBranch = SafetyBranchBase<2>;
type SafetyDomain = SafetyDomainBase<2>;
type SafetyPredicate = SafetyPredicateBase<2>;
type SafetyStep = SafetyStepBase<2>;
type TemporalFormula = TemporalFormulaBase<2>;
type Expression = Module["definitions"][number]["body"];
type Definition = Module["definitions"][number];
type Bound = Extract<
  Expression,
  { kind: "Forall" | "Exists" }
>["bounds"][number];
const sourceEdges = (value: Expression) =>
  analysisChildren({ space: "source", value }).flatMap((edge) =>
    edge.node.space === "source" ? [{ ...edge, value: edge.node.value }] : [],
  );
const chained = (
  value: Expression | undefined,
  kind: "And" | "Or",
): readonly Expression[] | undefined =>
  value && (value.kind === "And" || value.kind === "Or") && value.kind === kind
    ? value.operands
    : undefined;
const inherited = (value: Expression): readonly Expression[] =>
  sourceEdges(value)
    .filter((edge) => edge.forwarding.action === "inherit")
    .map((edge) => edge.value);
type StaticTarget =
  | { readonly kind: "slot"; readonly slot: number }
  | { readonly kind: "binding"; readonly binding: number }
  | { readonly kind: "tuple"; readonly values: readonly StaticTarget[] };
interface Scope {
  readonly entries: Map<
    string,
    {
      readonly definition: Definition;
      readonly staticTargets: ReadonlyMap<number, StaticTarget>;
      readonly targetActuals: ReadonlyMap<
        number,
        { readonly value: Expression; readonly context: Context }
      >;
      readonly locals: ReadonlyMap<string, SafetyExpression>;
      readonly bindings: ReadonlyMap<string, number>;
      readonly projection: ReadonlyMap<number, SafetyExpression>;
      readonly scope: Scope;
    }
  >;
}
interface AnonymousFrame {
  readonly arguments: readonly SafetyExpression[];
  readonly captures: readonly SafetyExpression[];
  readonly parameterCount: number;
  readonly captureCount: number;
}
interface Context {
  readonly staticTargets: ReadonlyMap<number, StaticTarget>;
  readonly targetActuals: ReadonlyMap<
    number,
    { readonly value: Expression; readonly context: Context }
  >;
  readonly projection: ReadonlyMap<number, SafetyExpression>;
  readonly bindings: ReadonlyMap<string, number>;
  readonly phase: "initial" | "next" | "state" | "constant";
  readonly locals: ReadonlyMap<string, SafetyExpression>;
  readonly calls: readonly string[];
  readonly depth: number;
  readonly bound: number;
  readonly scope: Scope;
}
const unmatchedCase = (value: Expression): SafetyExpression => ({
  op: "unmatched-case",
  location: `line ${value.location.line}, column ${value.location.column}`,
});
const getLevel = (value: unknown): number => {
  if (typeof value !== "object" || value === null) return 0;
  if ("op" in value && value.op === "read" && "next" in value)
    return value.next ? 2 : 1;
  if ("op" in value && value.op === "prime") return 2;
  if ("op" in value && value.op === "enabled") return 1;
  return Object.values(value).reduce<number>(
    (level, entry: unknown) => Math.max(level, getLevel(entry)),
    0,
  );
};

export const compileTlaSafety = (options: TlaSourceOptions): SafetyProgram => {
  if (options.syntaxVersion !== undefined && options.syntaxVersion !== "1.7.4")
    throw new PortableCoreError(
      "protocol",
      "Use the Rust frontend for syntaxVersion 1.8.0",
    );
  if (options.irVersion !== undefined && ![1, 2].includes(options.irVersion))
    throw new PortableCoreError("protocol", "Unsupported safety IR version");
  const shared = options.irVersion === 2;
  const module = parseModule(options.source);
  const config = parseConfig(options.config);
  const unsupported = (message: string): never => {
    throw new PortableCoreError(
      "protocol",
      `Unsupported TLA input: ${message}`,
    );
  };
  const labeling: { node: AnalysisNode; old: boolean }[] =
    module.definitions.map((definition) => ({
      node: { space: "source-definition", value: definition },
      old: false,
    }));
  while (labeling.length) {
    const task = labeling.pop();
    if (!task) break;
    if (
      task.old &&
      task.node.space === "source" &&
      task.node.value.kind === "Label"
    )
      unsupported("labels inside EXCEPT replacements are not supported by TLC");
    labeling.push(
      ...analysisChildren(task.node).map((edge) => ({
        node: edge.node,
        old: task.old || edge.scope === "old",
      })),
    );
  }
  const sourceChild = (value: Expression, role: string): Expression =>
    sourceEdges(value).find((edge) => edge.role === role)?.value ??
    unsupported(`source position ${role}`);

  if (
    module.modules.length ||
    module.instances.length ||
    module.extends.some(
      (name) =>
        ![
          "Naturals",
          "Integers",
          "Sequences",
          "FiniteSets",
          "Apalache",
        ].includes(name),
    )
  )
    unsupported("modules or EXTENDS");
  if (
    config.constraints.length ||
    config.actionConstraints.length ||
    config.substitutions.length ||
    config.symmetry ||
    config.view ||
    config.alias ||
    config.postconditions.length ||
    config.possible.length ||
    config.periodic ||
    config.rlReward
  )
    unsupported("constrained configuration");
  if (
    config.specification
      ? config.init || config.next
      : !config.init || !config.next
  )
    unsupported("use SPECIFICATION or INIT / NEXT");
  const variables = module.variables.map((value) => value.name);
  if (
    !variables.length ||
    variables.length > 256 ||
    new Set(variables).size !== variables.length ||
    module.variables.some((value) => value.arity !== 0)
  )
    unsupported("state variables");
  const slots = new Map(variables.map((name, index) => [name, index]));
  const scope: Scope = { entries: new Map() };
  const symbols = [
    ...variables,
    ...module.constants.map((value) => value.name),
    ...module.definitions.map((value) => value.name),
  ];
  if (
    new Set(symbols).size !== symbols.length ||
    module.definitions.some(
      (definition) =>
        definition.kind === "Operator" &&
        (definition.params.some((param) => symbols.includes(param.name)) ||
          new Set(definition.params.map((param) => param.name)).size !==
            definition.params.length),
    )
  )
    unsupported("conflicting symbols");
  for (const definition of module.definitions)
    scope.entries.set(definition.name, {
      definition,
      staticTargets: new Map(),
      targetActuals: new Map(),
      locals: new Map(),
      bindings: new Map(),
      projection: new Map(),
      scope,
    });
  const constants = new Map<string, ConfigValue>();
  for (const assignment of config.constants) {
    if (assignment.module || assignment.args || constants.has(assignment.name))
      unsupported("constant assignment");
    constants.set(assignment.name, assignment.value);
  }
  if (
    module.constants.some(
      (value) => value.arity !== 0 || !constants.has(value.name),
    ) ||
    [...constants.keys()].some(
      (name) => !module.constants.some((value) => value.name === name),
    )
  )
    unsupported("missing or extra constants");
  const literal = (value: ConfigValue): SafetyExpression => {
    if (value.kind === "Boolean")
      return { op: "literal", value: { type: "bool", value: value.value } };
    if (
      value.kind === "Number" &&
      Number.isInteger(value.value) &&
      value.value >= -2147483648 &&
      value.value <= 2147483647
    )
      return {
        op: "literal",
        value: { type: "int", value: String(value.value) },
      };
    if (value.kind === "String")
      return { op: "data", value: { type: "string", value: value.value } };
    if (value.kind === "Set")
      return { op: "set", values: value.elements.map(literal) };
    return unsupported(
      "literal (integers must fit signed 32 bits; model values are not strings)",
    );
  };
  const bool = (value: boolean) => literal({ kind: "Boolean", value });
  const string = (value: string): SafetyExpression => ({
    op: "data",
    value: { type: "string", value },
  });
  const nested = (context: Context): Context => {
    if (context.depth >= 40)
      return unsupported("expression or operator nesting exceeds 40");
    return { ...context, depth: context.depth + 1 };
  };
  const bodyOf = (definition: Definition): Expression =>
    definition.kind === "Operator"
      ? definition.body
      : {
          kind: "FunctionConstruction",
          bounds: definition.bounds,
          body: definition.body,
          location: definition.location,
        };
  const identity = (definition: Definition) =>
    `${definition.location.line}:${definition.location.column}`;
  const active = (definition: Definition, name: string) =>
    shared ? identity(definition) : name;
  const definitions: {
    readonly declaration: Definition;
    readonly phase: Context["phase"];
    readonly purpose: "value" | "membership-type";
    readonly signature: readonly StaticTarget[];
    captures: readonly number[];
    definition: SafetyExpressionDefinition | SafetyActionDefinition;
    pending?: { readonly body: Expression; readonly context: Context };
  }[] = [];
  const bindingIds = new Map<string, number>();
  const bindingAt = (key: string): number => {
    const found = bindingIds.get(key);
    if (found !== undefined) return found;
    const id = bindingIds.size;
    bindingIds.set(key, id);
    return id;
  };
  const definitionIds = new Map<string, number>();
  let lowering = false;
  let metadataReady = false;
  interface LevelSummary {
    readonly base: number;
    readonly parameters: ReadonlySet<number>;
    readonly captures: ReadonlySet<number>;
  }
  const levels = new Map<number, LevelSummary>();
  const summary = (value: unknown): LevelSummary => {
    const empty: LevelSummary = {
      base: 0,
      parameters: new Set(),
      captures: new Set(),
    };
    if (Array.isArray(value)) {
      const summaries = value.map(summary);
      return {
        base: Math.max(0, ...summaries.map((entry) => entry.base)),
        parameters: new Set(
          summaries.flatMap((entry) => [...entry.parameters]),
        ),
        captures: new Set(summaries.flatMap((entry) => [...entry.captures])),
      };
    }
    if (typeof value !== "object" || value === null) return empty;
    const node = analysisView(value);
    if (!node) return empty;
    const children = analysisChildren(node);
    const combine = (values: readonly LevelSummary[]): LevelSummary => ({
      base: Math.max(0, ...values.map((entry) => entry.base)),
      parameters: new Set(values.flatMap((entry) => [...entry.parameters])),
      captures: new Set(values.flatMap((entry) => [...entry.captures])),
    });
    if (
      ("op" in value && value.op === "frame") ||
      ("kind" in value && value.kind === "frame")
    ) {
      const body = combine(
        children
          .filter((edge) => edge.scope === "frame")
          .map((edge) => summary(edge.node.value)),
      );
      const actuals = children.filter((edge) => edge.role === "frame-actual");
      const captures = children.filter((edge) => edge.role === "frame-capture");
      return combine([
        { ...empty, base: body.base },
        ...[...body.parameters].map((slot) =>
          summary(actuals[slot]?.node.value),
        ),
        ...[...body.captures].map((slot) =>
          summary(captures[slot]?.node.value),
        ),
      ]);
    }
    if ("kind" in value && value.kind === "call" && "definition" in value) {
      const body =
        levels.get(Number(value.definition)) ??
        unsupported("operator metadata");
      const actuals = children.filter((edge) => edge.role === "call-actual");
      const captures = children.filter((edge) => edge.role === "call-capture");
      return combine([
        { ...empty, base: body.base },
        ...[...body.parameters].map((slot) =>
          summary(actuals[slot]?.node.value),
        ),
        ...[...body.captures].map((slot) =>
          summary(captures[slot]?.node.value),
        ),
      ]);
    }
    if ("op" in value) {
      if (value.op === "read" && "next" in value)
        return { ...empty, base: value.next ? 2 : 1 };
      if (value.op === "prime" || value.op === "enabled")
        return { ...empty, base: value.op === "prime" ? 2 : 1 };
      if (
        (value.op === "parameter" || value.op === "capture") &&
        "slot" in value
      )
        return {
          ...empty,
          [value.op === "parameter" ? "parameters" : "captures"]: new Set([
            Number(value.slot),
          ]),
        };
      if (
        value.op === "call" &&
        "definition" in value &&
        "arguments" in value &&
        "captures" in value
      ) {
        const id = Number(value.definition);
        const body = levels.get(id);
        if (!body) return unsupported("operator metadata");
        const args = children.filter((edge) => edge.role === "call-actual");
        const captures = children.filter(
          (edge) => edge.role === "call-capture",
        );
        return combine([
          { ...empty, base: body.base },
          ...[...body.parameters].map((slot) =>
            summary(args[slot]?.node.value),
          ),
          ...[...body.captures].map((slot) =>
            summary(captures[slot]?.node.value),
          ),
        ]);
      }
    }
    return combine(children.map((edge) => summary(edge.node.value)));
  };
  const level = (value: unknown) => {
    if (!shared) return getLevel(value);
    lowerPending();
    prepareLevels();
    return summary(value).base;
  };

  const visitNode = (
    node: AnalysisNode,
    transform: (value: Record<string, unknown>) => Record<string, unknown>,
    scoped: boolean,
  ): unknown => {
    const value = mapAnalysisChildren(node, (edge) =>
      scoped && edge.scope === "frame"
        ? edge.node.value
        : visitNode(edge.node, transform, scoped),
    );
    return typeof value === "object" && value !== null && !Array.isArray(value)
      ? transform(value as Record<string, unknown>)
      : value;
  };
  const visit = (
    value: unknown,
    transform: (value: Record<string, unknown>) => Record<string, unknown>,
  ): unknown => {
    if (Array.isArray(value))
      return value.map((entry) => visit(entry, transform));
    const node = analysisView(value);
    return node ? visitNode(node, transform, false) : value;
  };
  const visitScope = (
    value: unknown,
    transform: (value: Record<string, unknown>) => Record<string, unknown>,
  ): unknown => {
    if (Array.isArray(value))
      return value.map((entry) => visitScope(entry, transform));
    const node = analysisView(value);
    return node ? visitNode(node, transform, true) : value;
  };
  const localScopeOf = (body: unknown): number => {
    let localScope = 0;
    visitScope(body, (value) => {
      if (
        value.op === "bound" ||
        ("slot" in value && "domain" in value && value.kind !== "choose")
      )
        localScope = Math.max(localScope, Number(value.slot) + 1);
      if (value.op === "fold")
        localScope = Math.max(localScope, Number(value.accumulator) + 2);
      return value;
    });
    return localScope;
  };
  const expressionFrame = (
    frame: AnonymousFrame,
    body: SafetyExpression,
  ): SafetyExpression =>
    compactAnonymous({
      op: "frame",
      arguments: frame.arguments,
      captures: frame.captures,
      parameterCount: frame.parameterCount,
      captureCount: frame.captureCount,
      localScope: localScopeOf(body),
      body,
    } as SafetyExpression);
  const compactAnonymous = <Value>(input: Value): Value =>
    visit(input, (value) => {
      if (value.op !== "frame" && value.kind !== "frame") return value;
      const key =
        value.op === "frame"
          ? "body"
          : "branches" in value
            ? "branches"
            : "formula";
      const used = new Set<number>();
      visitScope(value[key], (node) => {
        if (node.op === "capture") used.add(Number(node.slot));
        return node;
      });
      const captures = value.captures as readonly SafetyExpression[];
      const mapping = new Map(
        captures
          .map((_, slot) => slot)
          .filter((slot) => used.has(slot))
          .map((slot, index) => [slot, index]),
      );
      return {
        ...value,
        captures: captures.filter((_, slot) => mapping.has(slot)),
        captureCount: mapping.size,
        localScope: localScopeOf(value[key]),
        [key]: visitScope(value[key], (node) =>
          node.op === "capture"
            ? { ...node, slot: mapping.get(Number(node.slot)) }
            : node,
        ),
      };
    }) as Value;
  const targetMessage = "generative UNCHANGED requires state variables";
  const targetExpression = (value: StaticTarget): SafetyExpression => {
    if (value.kind === "slot")
      return { op: "read", slot: value.slot, next: false };
    if (value.kind === "tuple")
      return { op: "tuple", values: value.values.map(targetExpression) };
    return unsupported(targetMessage);
  };
  const compareTarget = (left: StaticTarget, right: StaticTarget): number => {
    if (left.kind === "slot" && right.kind === "slot")
      return left.slot - right.slot;
    if (left.kind === "tuple" && right.kind === "tuple") {
      for (
        let index = 0;
        index < Math.min(left.values.length, right.values.length);
        index++
      ) {
        const a = left.values[index];
        const b = right.values[index];
        if (a && b) {
          const order = compareTarget(a, b);
          if (order) return order;
        }
      }
      return left.values.length - right.values.length;
    }
    return left.kind === "slot" ? -1 : 1;
  };
  const targetLeaves = (target: StaticTarget): number => {
    let count = 0;
    const pending = [target];
    while (pending.length) {
      const value = pending.pop();
      if (value?.kind === "tuple") pending.push(...value.values);
      else count++;
    }
    return count;
  };
  const resolveTarget = (
    source: Expression,
    context: Context,
    leafLimit = Number.POSITIVE_INFINITY,
  ): StaticTarget => {
    let leaves = 0;
    const leaf = (target: StaticTarget): void => {
      leaves += targetLeaves(target);
      if (leaves > leafLimit) unsupported("Safety branch exceeds 256 steps");
    };
    type Env = {
      readonly context: Context;
      readonly actuals: ReadonlyMap<
        number,
        {
          readonly value: Expression;
          readonly env: Env;
          readonly active: ReadonlySet<Definition>;
        }
      >;
    };
    type Task =
      | {
          readonly kind: "value";
          readonly value: Expression;
          readonly env: Env;
          readonly active: ReadonlySet<Definition>;
        }
      | { readonly kind: "tuple"; readonly count: number };
    const tasks: Task[] = [
      {
        kind: "value",
        value: source,
        env: { context, actuals: new Map() },
        active: new Set(),
      },
    ];
    const results: StaticTarget[] = [];
    let visited = 0;
    while (tasks.length) {
      if (visited++ === targetExpansionLimit)
        unsupported(`target expansion exceeds ${targetExpansionLimit} nodes`);
      const task = tasks.pop();
      if (!task) return unsupported(targetMessage);
      if (task.kind === "tuple") {
        results.push({
          kind: "tuple",
          values: results.splice(results.length - task.count, task.count),
        });
        continue;
      }
      const { value, env, active } = task;
      const context = env.context;
      if (context.depth >= 40)
        return unsupported("expression or operator nesting exceeds 40");
      const deeper = (): Env => ({
        ...env,
        context: { ...context, depth: context.depth + 1 },
      });
      if (value.kind === "Tuple") {
        const children = sourceEdges(value);
        const child = deeper();
        tasks.push({ kind: "tuple", count: children.length });
        for (const edge of children.reverse())
          tasks.push({ ...task, value: edge.value, env: child });
        continue;
      }
      if (value.kind === "Label") {
        tasks.push({
          ...task,
          value: sourceChild(value, "result"),
          env: deeper(),
        });
        continue;
      }
      if (value.kind === "Let") {
        tasks.push({
          ...task,
          value: sourceChild(value, "result"),
          env: { ...env, context: letContext(value, context) },
        });
        continue;
      }
      const name =
        value.kind === "Identifier"
          ? value.name
          : value.kind === "Apply"
            ? value.operator
            : undefined;
      if (!name) return unsupported(targetMessage);
      if (value.kind === "Identifier" && context.locals.has(name)) {
        const binding = context.bindings.get(name);
        if (binding === undefined) return unsupported(targetMessage);
        const actual = env.actuals.get(binding);
        if (actual) {
          tasks.push({ kind: "value", ...actual });
          continue;
        }
        const original = context.targetActuals.get(binding);
        if (original) {
          tasks.push({
            kind: "value",
            value: original.value,
            env: { context: original.context, actuals: new Map() },
            active: new Set(),
          });
          continue;
        }
        const mapped = context.staticTargets.get(binding);
        if (mapped) {
          leaf(mapped);
          results.push(mapped);
        } else {
          const local = context.locals.get(name);
          if (local?.op !== "parameter" && local?.op !== "capture")
            return unsupported(targetMessage);
          leaf({ kind: "binding", binding });
          results.push({ kind: "binding", binding });
        }
        continue;
      }
      const slot = value.kind === "Identifier" ? slots.get(name) : undefined;
      if (slot !== undefined) {
        leaf({ kind: "slot", slot });
        results.push({ kind: "slot", slot });
        continue;
      }
      const entry = context.scope.entries.get(name);
      if (!entry) return unsupported(targetMessage);
      if (entry.definition.isRecursive || active.has(entry.definition))
        return unsupported("recursive operator definitions");
      const params =
        entry.definition.kind === "Operator" ? entry.definition.params : [];
      const args =
        value.kind === "Apply"
          ? sourceEdges(value).map((edge) => edge.value)
          : [];
      if (
        params.length !== args.length ||
        params.some((param) => param.arity !== 0)
      )
        return unsupported(`operator ${name}`);
      const locals = new Map(entry.locals);
      const bindings = new Map(entry.bindings);
      const actuals = new Map(env.actuals);
      for (const [index, param] of params.entries()) {
        const binding = bindingAt(
          `parameter:${identity(entry.definition)}:${index}`,
        );
        const argument = args[index];
        if (!argument) return unsupported(targetMessage);
        locals.set(param.name, { op: "parameter", slot: index });
        bindings.set(param.name, binding);
        actuals.set(binding, { value: argument, env, active });
      }
      tasks.push({
        kind: "value",
        value: bodyOf(entry.definition),
        env: {
          context: {
            ...context,
            locals,
            bindings,
            staticTargets: entry.staticTargets,
            targetActuals: entry.targetActuals,
            scope: entry.scope,
            depth: context.depth + 1,
          },
          actuals,
        },
        active: new Set([...active, entry.definition]),
      });
    }
    return results[0] ?? unsupported(targetMessage);
  };
  const requirementCache = new Map<string, readonly number[]>();
  const targetRequirements = (
    name: string,
    context: Context,
    actionMode = false,
  ): readonly number[] => {
    type Frame = {
      readonly name: string;
      readonly context: Context;
      readonly action: boolean;
    };
    type Task =
      | { readonly kind: "call"; readonly frame: Frame }
      | {
          readonly kind: "scan";
          readonly value: Expression;
          readonly context: Context;
          readonly action: boolean;
          readonly required: number[];
        }
      | {
          readonly kind: "compose";
          readonly value: Expression;
          readonly context: Context;
          readonly action: boolean;
          readonly required: number[];
        }
      | {
          readonly kind: "finish";
          readonly key: string;
          readonly required: number[];
        };
    const tasks: Task[] = [
      { kind: "call", frame: { name, context, action: actionMode } },
    ];
    const active = new Set<string>();
    const add = (target: StaticTarget, required: number[]) => {
      const pending = [target];
      while (pending.length) {
        const value = pending.pop();
        if (value?.kind === "binding" && !required.includes(value.binding))
          required.push(value.binding);
        if (value?.kind === "tuple")
          for (let i = value.values.length - 1; i >= 0; i--) {
            const child = value.values[i];
            if (child) pending.push(child);
          }
      }
    };
    const keyFor = (name: string, context: Context, action: boolean) => {
      const entry =
        context.scope.entries.get(name) ?? unsupported(`operator ${name}`);
      return `${identity(entry.definition)}:${action}`;
    };
    while (tasks.length) {
      const task = tasks.pop();
      if (!task) return unsupported(targetMessage);
      if (task.kind === "finish") {
        requirementCache.set(task.key, task.required);
        active.delete(task.key);
        continue;
      }
      if (task.kind === "call") {
        const { name, context, action } = task.frame;
        const key = keyFor(name, context, action);
        if (requirementCache.has(key)) continue;
        if (active.has(key))
          return unsupported("recursive operator definitions");
        const entry =
          context.scope.entries.get(name) ?? unsupported(`operator ${name}`);
        const params =
          entry.definition.kind === "Operator" ? entry.definition.params : [];
        const locals = new Map(entry.locals);
        const bindings = new Map(entry.bindings);
        for (const [index, param] of params.entries()) {
          locals.set(param.name, { op: "parameter", slot: index });
          bindings.set(
            param.name,
            bindingAt(`parameter:${identity(entry.definition)}:${index}`),
          );
        }
        for (const [name, binding] of entry.bindings)
          locals.set(name, { op: "capture", slot: binding });
        const required: number[] = [];
        active.add(key);
        tasks.push({ kind: "finish", key, required });
        tasks.push({
          kind: "scan",
          value: bodyOf(entry.definition),
          context: {
            ...context,
            locals,
            bindings,
            scope: entry.scope,
            staticTargets: new Map(),
            targetActuals: new Map(),
            depth: 0,
          },
          action,
          required,
        });
        continue;
      }
      const { value, context, action, required } = task;
      const name =
        value.kind === "Identifier"
          ? value.name
          : value.kind === "Apply"
            ? value.operator
            : undefined;
      const entry =
        name && !context.locals.has(name)
          ? context.scope.entries.get(name)
          : undefined;
      if (task.kind === "compose") {
        if (!entry || !name) return unsupported(targetMessage);
        const params =
          entry.definition.kind === "Operator" ? entry.definition.params : [];
        const args =
          value.kind === "Apply"
            ? sourceEdges(value).map((edge) => edge.value)
            : [];
        for (const binding of requirementCache.get(
          keyFor(name, context, action),
        ) ?? []) {
          const index = params.findIndex(
            (_, index) =>
              bindingAt(`parameter:${identity(entry.definition)}:${index}`) ===
              binding,
          );
          if (index >= 0) {
            const argument = args[index];
            if (!argument) return unsupported(targetMessage);
            add(resolveTarget(argument, context), required);
          } else add({ kind: "binding", binding }, required);
        }
        continue;
      }
      if (entry && name) {
        if (value.kind === "Apply")
          for (const edge of sourceEdges(value).reverse())
            tasks.push({ ...task, value: edge.value, action: false });
        tasks.push({ ...task, kind: "compose" });
        tasks.push({ kind: "call", frame: { name, context, action } });
        continue;
      }
      if (
        action &&
        (value.kind === "BoxAction" || value.kind === "AngleAction")
      ) {
        add(resolveTarget(sourceChild(value, "subscript"), context), required);
        tasks.push({
          ...task,
          value: sourceChild(value, "action-body"),
          action: true,
        });
        continue;
      }
      if (value.kind === "Unchanged" && action) {
        add(
          resolveTarget(sourceChild(value, "generative-target"), context, 256),
          required,
        );
        continue;
      }
      if (value.kind === "Let") {
        tasks.push({
          ...task,
          value: sourceChild(value, "result"),
          context: letContext(value, context),
        });
        continue;
      }
      if (
        value.kind === "Forall" ||
        value.kind === "Exists" ||
        value.kind === "Choose" ||
        value.kind === "FunctionConstruction" ||
        value.kind === "SetMap" ||
        value.kind === "SetFilter"
      ) {
        const bounds =
          value.kind === "SetFilter"
            ? [value.bound]
            : value.kind === "Choose"
              ? value.set
                ? [{ pattern: value.pattern, set: value.set }]
                : []
              : value.bounds;
        const locals = new Map(context.locals);
        const bindings = new Map(context.bindings);
        for (const bound of bounds)
          for (const name of bound.pattern.kind === "Name"
            ? [bound.pattern.name]
            : bound.pattern.names) {
            const binding = bindingAt(
              `binder:${bound.set.location.line}:${bound.set.location.column}:${name}`,
            );
            locals.set(name, { op: "bound", slot: binding });
            bindings.set(name, binding);
          }
        for (const edge of sourceEdges(value).reverse())
          tasks.push({
            ...task,
            value: edge.value,
            context:
              edge.scope === "binder"
                ? { ...context, locals, bindings }
                : context,
            action:
              edge.forwarding.action === "start" ||
              (action && edge.forwarding.action === "inherit"),
          });
        continue;
      }
      const children = sourceEdges(value);
      for (let index = children.length - 1; index >= 0; index--) {
        const edge = children[index];
        if (edge)
          tasks.push({
            ...task,
            value: edge.value,
            action:
              edge.forwarding.action === "start" ||
              (action && edge.forwarding.action === "inherit"),
          });
      }
    }
    return requirementCache.get(keyFor(name, context, actionMode)) ?? [];
  };
  const membershipCache = new Map<string, readonly number[]>();
  const membershipRequirements = (
    name: string,
    context: Context,
  ): readonly number[] => {
    type Task =
      | {
          readonly kind: "call";
          readonly name: string;
          readonly context: Context;
          readonly membership: boolean;
        }
      | {
          readonly kind: "scan" | "compose";
          readonly value: Expression;
          readonly context: Context;
          readonly membership: boolean;
          readonly required: number[];
        }
      | {
          readonly kind: "finish";
          readonly key: string;
          readonly required: number[];
        };
    const keyFor = (name: string, context: Context, membership: boolean) =>
      `${identity((context.scope.entries.get(name) ?? unsupported(`operator ${name}`)).definition)}:${membership}`;
    const tasks: Task[] = [{ kind: "call", name, context, membership: false }];
    const active = new Set<string>();
    const add = (binding: number, required: number[]) => {
      if (!required.includes(binding)) required.push(binding);
    };
    while (tasks.length) {
      const task = tasks.pop();
      if (!task) return unsupported("operator membership metadata");
      if (task.kind === "finish") {
        membershipCache.set(task.key, task.required);
        active.delete(task.key);
        continue;
      }
      if (task.kind === "call") {
        const { name, context, membership } = task;
        const key = keyFor(name, context, membership);
        if (membershipCache.has(key)) continue;
        if (active.has(key))
          return unsupported("recursive operator definitions");
        const entry =
          context.scope.entries.get(name) ?? unsupported(`operator ${name}`);
        const locals = new Map(entry.locals);
        const bindings = new Map(entry.bindings);
        const params =
          entry.definition.kind === "Operator" ? entry.definition.params : [];
        for (const [index, param] of params.entries()) {
          locals.set(param.name, { op: "parameter", slot: index });
          bindings.set(
            param.name,
            bindingAt(`parameter:${identity(entry.definition)}:${index}`),
          );
        }
        const required: number[] = [];
        active.add(key);
        tasks.push({ kind: "finish", key, required });
        tasks.push({
          kind: "scan",
          value: bodyOf(entry.definition),
          context: {
            ...context,
            locals,
            bindings,
            scope: entry.scope,
            depth: 0,
          },
          membership,
          required,
        });
        continue;
      }
      const { value, context, membership, required } = task;
      const name =
        value.kind === "Identifier"
          ? value.name
          : value.kind === "Apply"
            ? value.operator
            : undefined;
      const entry =
        name && !context.locals.has(name)
          ? context.scope.entries.get(name)
          : undefined;
      if (task.kind === "compose") {
        if (!entry || !name) return unsupported("operator membership metadata");
        const params =
          entry.definition.kind === "Operator" ? entry.definition.params : [];
        const args =
          value.kind === "Apply"
            ? sourceEdges(value).map((edge) => edge.value)
            : [];
        for (const binding of membershipCache.get(
          keyFor(name, context, membership),
        ) ?? []) {
          const index = params.findIndex(
            (_, index) =>
              bindingAt(`parameter:${identity(entry.definition)}:${index}`) ===
              binding,
          );
          if (index >= 0) {
            const argument = args[index];
            if (argument)
              tasks.push({
                ...task,
                kind: "scan",
                value: argument,
                membership: true,
              });
          } else add(binding, required);
        }
        continue;
      }
      if (value.kind === "Identifier" && context.locals.has(value.name)) {
        const binding = context.bindings.get(value.name);
        if (membership && binding !== undefined) add(binding, required);
        continue;
      }
      if (entry && name) {
        if (value.kind === "Apply")
          for (const edge of sourceEdges(value).reverse())
            tasks.push({ ...task, value: edge.value, membership: false });
        tasks.push({ ...task, kind: "compose" });
        tasks.push({ kind: "call", name, context, membership });
        continue;
      }
      if (value.kind === "Let") {
        tasks.push({
          ...task,
          value: sourceChild(value, "result"),
          context: letContext(value, context),
        });
        continue;
      }
      if (
        value.kind === "Forall" ||
        value.kind === "Exists" ||
        value.kind === "FunctionConstruction" ||
        value.kind === "SetMap" ||
        value.kind === "SetFilter" ||
        value.kind === "Choose"
      ) {
        const locals = new Map(context.locals);
        const bindings = new Map(context.bindings);
        const bounds =
          value.kind === "SetFilter"
            ? [value.bound]
            : value.kind === "Choose"
              ? value.set
                ? [{ pattern: value.pattern, set: value.set }]
                : []
              : value.bounds;
        for (const bound of bounds)
          for (const name of bound.pattern.kind === "Name"
            ? [bound.pattern.name]
            : bound.pattern.names) {
            locals.set(name, { op: "bound", slot: 0 });
            bindings.set(
              name,
              bindingAt(
                `binder:${bound.set.location.line}:${bound.set.location.column}:${name}`,
              ),
            );
          }
        for (const edge of sourceEdges(value).reverse())
          tasks.push({
            ...task,
            value: edge.value,
            context:
              edge.scope === "binder"
                ? { ...context, locals, bindings }
                : context,
            membership:
              edge.forwarding.membership === "start" ||
              (membership && edge.forwarding.membership === "inherit"),
          });
        continue;
      }
      const children = sourceEdges(value);
      for (let index = children.length - 1; index >= 0; index--) {
        const edge = children[index];
        if (edge)
          tasks.push({
            ...task,
            value: edge.value,
            context,
            membership:
              edge.forwarding.membership === "start" ||
              (membership && edge.forwarding.membership === "inherit"),
          });
      }
    }
    return membershipCache.get(keyFor(name, context, false)) ?? [];
  };
  const sharedCall = (
    name: string,
    args: readonly Expression[],
    context: Context,
    purpose: "value" | "membership-type" = "value",
    lowered?: readonly SafetyExpression[],
    kind: "expression" | "action" = "expression",
  ): SafetyExpression => {
    const entry = context.scope.entries.get(name);
    if (!entry || entry.definition.isRecursive)
      return unsupported(`operator ${name}`);
    const declaration = entry.definition;
    const params = declaration.kind === "Operator" ? declaration.params : [];
    if (
      params.length !== (lowered?.length ?? args.length) ||
      params.some((param) => param.arity !== 0)
    )
      return unsupported(`operator ${name}`);
    const required = targetRequirements(name, context, kind === "action");
    const resolved = new Map<number, StaticTarget>();
    const signature = required.map((binding) => {
      const index = params.findIndex(
        (_, index) =>
          bindingAt(`parameter:${identity(declaration)}:${index}`) === binding,
      );
      const argument = index >= 0 ? args[index] : undefined;
      const captured =
        context.targetActuals.get(binding) ?? entry.targetActuals.get(binding);
      const target = argument
        ? resolveTarget(argument, context)
        : captured
          ? resolveTarget(captured.value, captured.context)
          : (context.staticTargets.get(binding) ??
            entry.staticTargets.get(binding));
      if (!target) return unsupported(targetMessage);
      targetExpression(target);
      resolved.set(binding, target);
      return target;
    });
    const key = `${identity(declaration)}:${context.phase}:${purpose}:${JSON.stringify(signature)}:${kind}`;
    const saved = definitionIds.get(key);
    let found = saved === undefined ? undefined : definitions[saved];
    if (!found) {
      const captures = [...entry.projection.keys()];
      const references = new Map<number, SafetyExpression>(
        captures.map((binding, slot) => [binding, { op: "capture", slot }]),
      );
      const locals = new Map<string, SafetyExpression>(
        [...entry.bindings].flatMap(([name, binding]) => {
          const value = references.get(binding);
          return value ? [[name, value] as const] : [];
        }),
      );
      const bindings = new Map(entry.bindings);
      const projection = new Map(references);
      for (const [slot, param] of params.entries()) {
        const binding = bindingAt(`parameter:${identity(declaration)}:${slot}`);
        const value: SafetyExpression = { op: "parameter", slot };
        locals.delete(param.name);
        locals.set(param.name, value);
        bindings.set(param.name, binding);
        projection.set(binding, value);
      }
      const frame: Context = {
        staticTargets: resolved,
        targetActuals: new Map(),
        projection,
        bindings,
        phase: context.phase,
        locals,
        scope: entry.scope,
        calls: [],
        bound: 0,
        depth: 0,
      };
      found = {
        declaration,
        phase: context.phase,
        purpose,
        signature,
        captures,
        definition:
          kind === "action"
            ? {
                id: definitions.length,
                parameterCount: params.length,
                captureCount: captures.length,
                bodyKind: "action",
                localScope: 0,
                body: [],
              }
            : {
                id: definitions.length,
                parameterCount: params.length,
                captureCount: captures.length,
                bodyKind: "expression",
                localScope: 0,
                body: bool(true),
              },
        pending: { body: bodyOf(declaration), context: frame },
      };
      definitionIds.set(key, definitions.length);
      definitions.push(found);
      metadataReady = false;
    }
    const membership = new Set(membershipRequirements(name, context));
    return {
      op: "call",
      definition: found.definition.id,
      arguments:
        lowered ??
        args.map((argument, index) =>
          (purpose === "membership-type" ||
            membership.has(
              bindingAt(`parameter:${identity(declaration)}:${index}`),
            )
            ? membershipSet
            : expression)(argument, nested(context)),
        ),
      captures: found.captures.map(
        (binding) =>
          context.projection.get(binding) ??
          entry.projection.get(binding) ??
          unsupported("operator capture"),
      ),
    };
  };
  const checkDepth = (value: unknown): void => {
    const root = analysisView(value);
    if (!root) return;
    const pending: (readonly [AnalysisNode, number, number])[] = [[root, 0, 0]];
    while (pending.length) {
      const task = pending.pop();
      if (!task) break;
      const [node, depth, temporalDepth] = task;
      if (node.space === "ir-temporal" && temporalDepth > 128)
        unsupported("compiled temporal nesting exceeds 128");
      if (
        (node.space === "ir-expression" ||
          ((node.space === "ir-step" || node.space === "ir-temporal") &&
            node.value.kind === "frame")) &&
        depth > 128
      )
        unsupported("compiled expression nesting exceeds 128");
      for (const edge of analysisChildren(node))
        pending.push([
          edge.node,
          depth + edge.depth,
          temporalDepth + Number(node.space === "ir-temporal"),
        ]);
    }
  };
  const lowerDefinition = (entry: (typeof definitions)[number]): void => {
    if (!entry.pending) return;
    const { body, context } = entry.pending;
    delete entry.pending;
    if (entry.definition.bodyKind === "action") {
      const body = action(bodyOf(entry.declaration), context, "<action>");
      entry.definition = {
        ...entry.definition,
        body,
        localScope: localScopeOf(body),
      };
      metadataReady = false;
      return;
    }
    const lowered =
      entry.purpose === "membership-type"
        ? membershipSet(body, context)
        : expression(body, context);
    checkDepth(lowered);
    entry.definition = {
      ...entry.definition,
      body: lowered,
      localScope: localScopeOf(lowered),
    };
    metadataReady = false;
  };
  const lowerPending = (): void => {
    if (!shared || lowering) return;
    lowering = true;
    for (let id = 0; id < definitions.length; id++) {
      const entry = definitions[id];
      if (!entry?.pending) continue;
      lowerDefinition(entry);
    }
    lowering = false;
    prepareLevels();
  };
  const prepareLevels = (): void => {
    if (metadataReady) return;
    levels.clear();
    const edges = definitions.map((entry) => {
      const calls = new Set<number>();
      visit(entry.definition.body, (value) => {
        if (value.op === "call" || value.kind === "call")
          calls.add(Number(value.definition));
        return value;
      });
      return [...calls];
    });
    const states = new Uint8Array(definitions.length);
    for (let root = 0; root < definitions.length; root++) {
      if (states[root] === 2) continue;
      const stack = [{ id: root, next: 0 }];
      states[root] = 1;
      while (stack.length) {
        const frame =
          stack[stack.length - 1] ?? unsupported("operator metadata");
        const dependencies =
          edges[frame.id] ?? unsupported("operator metadata");
        const dependency = dependencies[frame.next++];
        if (dependency !== undefined) {
          if (states[dependency] === 1)
            unsupported("recursive operator definitions");
          if (states[dependency] !== 2) {
            if (!definitions[dependency]) unsupported("operator metadata");
            states[dependency] = 1;
            stack.push({ id: dependency, next: 0 });
          }
          continue;
        }
        const entry = definitions[frame.id] ?? unsupported("operator metadata");
        const value = summary(entry.definition.body);
        levels.set(
          frame.id,
          entry.definition.bodyKind === "action"
            ? {
                ...value,
                base: Math.max(value.base, entry.phase === "initial" ? 1 : 2),
              }
            : value,
        );
        states[frame.id] = 2;
        stack.pop();
      }
    }
    metadataReady = true;
  };
  const compactCaptures = <Value>(program: Value): Value => {
    let result = compactAnonymous(program);
    for (;;) {
      for (const entry of definitions)
        entry.definition = {
          ...entry.definition,
          body: compactAnonymous(entry.definition.body),
        } as typeof entry.definition;
      const mappings = definitions.map((entry) => {
        const used = new Set<number>();
        visitScope(entry.definition.body, (value) => {
          if (value.op === "capture") used.add(Number(value.slot));
          return value;
        });
        return new Map(
          entry.captures
            .map((_, slot) => slot)
            .filter((slot) => used.has(slot))
            .map((slot, index) => [slot, index]),
        );
      });
      if (
        definitions.every(
          (entry, id) => entry.captures.length === mappings[id]?.size,
        )
      )
        break;
      const trimCalls = (value: Record<string, unknown>) =>
        value.op === "call" || value.kind === "call"
          ? {
              ...value,
              captures: (value.captures as readonly SafetyExpression[]).filter(
                (_, slot) => mappings[Number(value.definition)]?.has(slot),
              ),
            }
          : value;
      for (const [id, entry] of definitions.entries()) {
        const mapping = mappings[id];
        if (!mapping) return unsupported("operator capture");
        entry.captures = entry.captures.filter((_, slot) => mapping.has(slot));
        const body = visitScope(entry.definition.body, (value) =>
          value.op === "capture"
            ? { ...value, slot: mapping.get(Number(value.slot)) }
            : value,
        );
        entry.definition = {
          ...entry.definition,
          captureCount: entry.captures.length,
          body: visit(body, trimCalls),
        } as typeof entry.definition;
      }
      result = compactAnonymous(visit(result, trimCalls) as Value);
    }
    metadataReady = false;
    prepareLevels();
    const validatePrime = (value: Record<string, unknown>) => {
      if (value.op === "prime" && summary(value.argument).base > 1)
        return unsupported("prime of an action expression");
      return value;
    };
    for (const entry of definitions)
      visit(entry.definition.body, validatePrime);
    visit(result, validatePrime);
    return result;
  };
  const call = (
    name: string,
    args: readonly Expression[],
    context: Context,
    lowered?: readonly SafetyExpression[],
  ) => {
    const entry = context.scope.entries.get(name);
    const definition = entry?.definition;
    if (
      !entry ||
      !definition ||
      definition.isRecursive ||
      context.calls.includes(active(definition, name))
    )
      return unsupported(`operator ${name}`);
    if (
      !shared &&
      args.some(hasFreeOld) &&
      rebindsOld(bodyOf(definition), entry.scope)
    )
      return unsupported(
        "EXCEPT @ arguments across a nested EXCEPT require safety IR v2",
      );
    if (shared) {
      const params = definition.kind === "Operator" ? definition.params : [];
      if (
        params.length !== (lowered?.length ?? args.length) ||
        params.some((param) => param.arity !== 0)
      )
        return unsupported(`operator ${name}`);
      const membership = new Set(membershipRequirements(name, context));
      const captures = [...entry.projection.keys()];
      const captureValues = captures.map(
        (binding) =>
          context.projection.get(binding) ??
          entry.projection.get(binding) ??
          unsupported("operator capture"),
      );
      const projection = new Map<number, SafetyExpression>(
        captures.map((binding, slot) => [binding, { op: "capture", slot }]),
      );
      const locals = new Map<string, SafetyExpression>(
        [...entry.bindings].flatMap(([name, binding]) => {
          const value = projection.get(binding);
          return value ? [[name, value] as const] : [];
        }),
      );
      const bindings = new Map(entry.bindings);
      const targetActuals = new Map([
        ...entry.targetActuals,
        ...context.targetActuals,
      ]);
      for (const [slot, param] of params.entries()) {
        const binding = bindingAt(`parameter:${identity(definition)}:${slot}`);
        const value: SafetyExpression = { op: "parameter", slot };
        locals.delete(param.name);
        locals.set(param.name, value);
        bindings.set(param.name, binding);
        projection.set(binding, value);
        const argument = args[slot];
        if (argument) targetActuals.set(binding, { value: argument, context });
      }
      return {
        body: bodyOf(definition),
        context: {
          ...nested(context),
          staticTargets: new Map([
            ...entry.staticTargets,
            ...context.staticTargets,
          ]),
          targetActuals,
          projection,
          bindings,
          locals,
          scope: entry.scope,
          calls: [...context.calls, active(definition, name)],
          bound: 0,
        },
        frame:
          params.length || captures.length || context.bound > 0
            ? {
                parameterCount: params.length,
                captureCount: captures.length,
                arguments:
                  lowered ??
                  args.map((argument, index) =>
                    (membership.has(
                      bindingAt(`parameter:${identity(definition)}:${index}`),
                    )
                      ? membershipSet
                      : expression)(argument, context),
                  ),
                captures: captureValues,
              }
            : undefined,
      };
    }
    const bindings = new Map(entry.bindings);
    const staticTargets = new Map([
      ...entry.staticTargets,
      ...context.staticTargets,
    ]);
    const targetActuals = new Map([
      ...entry.targetActuals,
      ...context.targetActuals,
    ]);
    const projection = new Map([...entry.projection, ...context.projection]);
    const locals = new Map(
      [...entry.locals].map(([name, value]) => [name, value]),
    );
    if (definition.kind === "Operator") {
      if (
        definition.params.length !== (lowered?.length ?? args.length) ||
        definition.params.some((param) => param.arity !== 0)
      )
        return unsupported(`operator ${name}`);
      for (const [index, param] of definition.params.entries()) {
        const argument = args[index];
        const value =
          lowered?.[index] ??
          (argument
            ? expression(argument, context)
            : unsupported(`operator argument ${name}`));
        locals.set(param.name, value);
        const binding = bindingAt(`parameter:${identity(definition)}:${index}`);
        bindings.set(param.name, binding);
        projection.set(binding, value);
      }
    } else if (args.length) return unsupported(`function operator ${name}`);
    return {
      frame: undefined,
      body:
        definition.kind === "Operator"
          ? definition.body
          : ({
              kind: "FunctionConstruction",
              bounds: definition.bounds,
              body: definition.body,
              location: definition.location,
            } satisfies Expression),
      context: {
        ...nested(context),
        staticTargets,
        targetActuals,
        locals,
        bindings,
        projection,
        scope: entry.scope,
        calls: [...context.calls, active(definition, name)],
      },
    };
  };
  const fold = (
    sequence: boolean,
    args: readonly Expression[],
    context: Context,
    child: Context,
  ): SafetyExpression => {
    const [operator, initial, collection] = args;
    if (args.length !== 3 || !initial || !collection)
      return unsupported("fold arity");
    if (operator?.kind !== "Identifier")
      return unsupported("fold operator argument");
    const name = operator.name;
    const entry = context.scope.entries.get(name);
    const definition = entry?.definition;
    if (
      context.locals.has(name) ||
      !entry ||
      !definition ||
      definition.isRecursive ||
      context.calls.includes(active(definition, name)) ||
      definition.kind !== "Operator" ||
      definition.params.length !== 2 ||
      definition.params.some((param) => param.arity !== 0)
    )
      return unsupported(`fold operator ${name}`);
    const accumulator = context.bound;
    if (accumulator + 2 > 40)
      return unsupported("more than 40 bound variables");
    const locals = new Map(entry.locals);
    for (const [index, param] of definition.params.entries())
      locals.set(param.name, { op: "bound", slot: accumulator + index });
    return {
      op: "fold",
      accumulator,
      initial: expression(initial, child),
      collection: expression(collection, child),
      order: sequence ? "sequence-left" : "set-members",
      body: shared
        ? sharedCall(name, [], context, "value", [
            { op: "bound", slot: accumulator },
            { op: "bound", slot: accumulator + 1 },
          ])
        : expression(definition.body, {
            ...nested(child),
            locals,
            scope: entry.scope,
            calls: [...child.calls, active(definition, name)],
            bound: accumulator + 2,
          }),
    };
  };
  const hasFreeOld = (body: Expression): boolean => {
    const pending: { readonly node: AnalysisNode; readonly bound: boolean }[] =
      [{ node: { space: "source", value: body }, bound: false }];
    while (pending.length) {
      const task = pending.pop();
      if (!task) break;
      const { node, bound } = task;
      if (node.space === "source" && node.value.kind === "ExceptAt" && !bound)
        return true;
      for (const edge of analysisChildren(node))
        pending.push({ node: edge.node, bound: bound || edge.scope === "old" });
    }
    return false;
  };
  const rebindsOld = (body: Expression, scope: Scope): boolean => {
    const visited = new Set<Definition>();
    const pending: { node: AnalysisNode; scope: Scope }[] = [
      { node: { space: "source", value: body }, scope },
    ];
    while (pending.length) {
      const task = pending.pop();
      if (!task) break;
      if (task.node.space === "source") {
        const value = task.node.value;
        if (value.kind === "Except") return true;
        const name =
          value.kind === "Identifier"
            ? value.name
            : value.kind === "Apply"
              ? value.operator
              : undefined;
        const callee = name ? task.scope.entries.get(name) : undefined;
        if (callee && !visited.has(callee.definition)) {
          visited.add(callee.definition);
          pending.push({
            node: { space: "source", value: bodyOf(callee.definition) },
            scope: callee.scope,
          });
        }
      }
      pending.push(
        ...analysisChildren(task.node).map((edge) => ({
          node: edge.node,
          scope: task.scope,
        })),
      );
    }
    return false;
  };
  const letContext = (
    value: Extract<Expression, { kind: "Let" }>,
    context: Context,
  ): Context => {
    if (value.instances?.length) return unsupported("LET INSTANCE");
    const childScope: Scope = { entries: new Map(context.scope.entries) };
    if (
      new Set(value.definitions.map((definition) => definition.name)).size !==
      value.definitions.length
    )
      return unsupported("duplicate LET definitions");
    for (const definition of value.definitions)
      childScope.entries.set(definition.name, {
        definition,
        staticTargets: context.staticTargets,
        targetActuals: context.targetActuals,
        locals: context.locals,
        bindings: context.bindings,
        projection: context.projection,
        scope: childScope,
      });
    if (
      value.definitions.some(
        (definition) =>
          (definition.kind === "Function" || definition.params.length === 0) &&
          hasFreeOld(bodyOf(definition)),
      ) &&
      rebindsOld(value.body, childScope)
    )
      return unsupported(
        "zero-arity LET @ across a nested EXCEPT requires the Rust frontend",
      );
    return { ...nested(context), scope: childScope };
  };
  const bind = (bounds: readonly Bound[], context: Context) => {
    const locals = new Map(context.locals);
    const lexicalBindings = new Map(context.bindings);
    const projection = new Map(context.projection);
    const bindings: SafetyBinding[] = [];
    let child = context;
    for (const bound of bounds) {
      if (child.bound >= 40) return unsupported("more than 40 bound variables");
      const slot = child.bound;
      bindings.push({
        slot,
        domain: domain(bound.set, context, true),
        ...(bound.pattern.kind === "Tuple"
          ? { tupleSize: bound.pattern.names.length }
          : {}),
      });
      if (shared)
        for (const name of bound.pattern.kind === "Name"
          ? [bound.pattern.name]
          : bound.pattern.names)
          locals.delete(name);
      if (bound.pattern.kind === "Name")
        locals.set(bound.pattern.name, { op: "bound", slot });
      else
        for (const [index, name] of bound.pattern.names.entries())
          locals.set(name, {
            op: "at",
            target: { op: "bound", slot },
            key: literal({ kind: "Number", value: index + 1 }),
          });
      for (const name of bound.pattern.kind === "Name"
        ? [bound.pattern.name]
        : bound.pattern.names) {
        const binding = bindingAt(
          `binder:${bound.set.location.line}:${bound.set.location.column}:${name}`,
        );
        lexicalBindings.set(name, binding);
        const value = locals.get(name);
        if (value) projection.set(binding, value);
      }
      child = {
        ...nested(child),
        locals: new Map(locals),
        bindings: new Map(lexicalBindings),
        projection: new Map(projection),
        bound: slot + 1,
      };
    }
    return { bindings, context: child };
  };
  const hasArithmetic = module.extends.some((name) =>
    ["Naturals", "Integers", "Sequences", "FiniteSets"].includes(name),
  );
  const domain = (
    value: Expression,
    context: Context,
    choice = false,
  ): SafetyDomain => {
    if (value.kind === "SetEnumeration")
      return {
        kind: "set",
        values: value.elements.map((entry) =>
          expression(entry, nested(context)),
        ),
      };
    if (
      value.kind === "Apply" &&
      value.operator === ".." &&
      value.args[0] &&
      value.args[1]
    ) {
      if (!hasArithmetic) return unsupported("range requires Naturals");
      return {
        kind: "range",
        lower: expression(value.args[0], nested(context)),
        upper: expression(value.args[1], nested(context)),
      };
    }
    if (value.kind === "Identifier") {
      if (value.name === "BOOLEAN")
        return { kind: "set", values: [bool(false), bool(true)] };
      if (value.name === "Nat" || value.name === "Int") {
        if (
          choice ||
          !hasArithmetic ||
          (value.name === "Int" && !module.extends.includes("Integers"))
        )
          return unsupported("infinite enumeration");
        return {
          kind: "range",
          lower: literal({
            kind: "Number",
            value: value.name === "Nat" ? 0 : -2147483648,
          }),
          upper: literal({ kind: "Number", value: 2147483647 }),
        };
      }
      if (
        !shared &&
        context.scope.entries.has(value.name) &&
        !context.locals.has(value.name)
      ) {
        const expanded = call(value.name, [], context);
        return domain(expanded.body, expanded.context, choice);
      }
    }
    return { kind: "value", value: expression(value, context) };
  };
  const binary = new Map<
    string,
    | "add"
    | "sub"
    | "mul"
    | "eq"
    | "ne"
    | "lt"
    | "le"
    | "gt"
    | "ge"
    | "implies"
    | "cup"
    | "cap"
    | "difference"
    | "subset-eq"
    | "concat"
  >([
    ["+", "add"],
    ["-", "sub"],
    ["*", "mul"],
    ["=", "eq"],
    ["#", "ne"],
    ["/=", "ne"],
    ["<", "lt"],
    ["<=", "le"],
    ["\\leq", "le"],
    [">", "gt"],
    [">=", "ge"],
    ["\\geq", "ge"],
    ["=>", "implies"],
    ["\\cup", "cup"],
    ["\\union", "cup"],
    ["\\cap", "cap"],
    ["\\intersect", "cap"],
    ["\\", "difference"],
    ["\\subseteq", "subset-eq"],
    ["\\o", "concat"],
  ]);
  const prime = (value: SafetyExpression): SafetyExpression => {
    if (level(value) > 1) return unsupported("prime of an action expression");
    return { op: "prime", argument: value };
  };
  const oldContext = (context: Context, expression: Expression): Context => {
    const binding = bindingAt(
      `old:${expression.location.line}:${expression.location.column}`,
    );
    const value: SafetyExpression = { op: "old" };
    const locals = new Map(context.locals);
    locals.delete("@");
    locals.set("@", value);
    return {
      ...context,
      locals,
      bindings: new Map([...context.bindings, ["@", binding]]),
      projection: new Map([...context.projection, [binding, value]]),
    };
  };
  const expression = (
    value: Expression,
    context: Context,
  ): SafetyExpression => {
    const child = nested(context);
    if (
      value.kind === "String" &&
      [...value.value].some(
        (character) => (character.codePointAt(0) ?? 0) > 255,
      )
    )
      return unsupported(
        "TLC 1.7.4 string literal contains a character outside the byte range",
      );
    if (value.kind === "Number" || value.kind === "String")
      return literal(value);
    if (value.kind === "Identifier") {
      const local = context.locals.get(value.name);
      if (local) {
        return local;
      }
      const constant = constants.get(value.name);
      if (constant) return literal(constant);
      if (value.name === "TRUE" || value.name === "FALSE")
        return bool(value.name === "TRUE");
      if (value.name === "BOOLEAN")
        return { op: "set", values: [bool(false), bool(true)] };
      const slot = slots.get(value.name);
      if (slot !== undefined) {
        if (context.phase === "constant")
          return unsupported("variable in constant expression");
        return { op: "read", slot, next: context.phase === "initial" };
      }
      if (shared) return sharedCall(value.name, [], context);
      const expanded = call(value.name, [], context);
      return expression(expanded.body, expanded.context);
    }
    if (value.kind === "Let")
      return expression(value.body, letContext(value, context));
    if (value.kind === "Label") return expression(value.body, child);
    if (value.kind === "Prime" && context.phase === "next") {
      const inner = expression(value.expression, child);
      return inner.op === "read" && !inner.next
        ? { ...inner, next: true }
        : prime(inner);
    }
    if (value.kind === "Unchanged" && context.phase === "next") {
      const inner = expression(value.expression, child);
      return { op: "eq", left: inner, right: prime(inner) };
    }
    if (value.kind === "Enabled")
      return {
        op: "enabled",
        branches: action(
          value.expression,
          { ...child, phase: "next" },
          "ENABLED",
        ),
      };
    if (value.kind === "ExceptAt") {
      const local = context.locals.get("@");
      return local ?? { op: "old" };
    }
    if (value.kind === "SetEnumeration" || value.kind === "Tuple")
      return {
        op: value.kind === "Tuple" ? "tuple" : "set",
        values: value.elements.map((entry) => expression(entry, child)),
      };
    if (value.kind === "Record" || value.kind === "RecordSet")
      return {
        op: value.kind === "Record" ? "record" : "record-set",
        fields: value.fields.map((field) => [
          field.name,
          expression(field.value, child),
        ]),
      };
    if (value.kind === "FunctionSet")
      return {
        op: "function-set",
        left: expression(value.domain, child),
        right: expression(value.range, child),
      };
    if (value.kind === "FunctionApplication")
      return {
        op: "at",
        target: expression(value.function, child),
        key:
          value.args.length === 1 && value.args[0]
            ? expression(value.args[0], child)
            : {
                op: "tuple",
                values: value.args.map((argument) =>
                  expression(argument, child),
                ),
              },
      };
    if (value.kind === "FieldAccess")
      return {
        op: "at",
        target: expression(value.record, child),
        key: string(value.field),
      };
    if (value.kind === "Except")
      return {
        op: "update",
        target: expression(value.function, child),
        updates: value.updates.map((update) => ({
          path: update.path.map((part) =>
            part.kind === "Field"
              ? string(part.name)
              : part.args.length === 1 && part.args[0]
                ? expression(part.args[0], child)
                : {
                    op: "tuple",
                    values: part.args.map((argument) =>
                      expression(argument, child),
                    ),
                  },
          ),
          value: expression(
            update.value,
            shared ? oldContext(child, update.value) : child,
          ),
        })),
      };
    if (shared && value.kind === "Choose" && value.set) {
      const bound = bind([{ pattern: value.pattern, set: value.set }], child);
      const binding = bound.bindings[0] ?? unsupported("CHOOSE binding");
      return {
        op: "choose",
        binding,
        body: expression(value.body, bound.context),
      };
    }
    if (
      value.kind === "Forall" ||
      value.kind === "Exists" ||
      value.kind === "FunctionConstruction" ||
      value.kind === "SetMap" ||
      value.kind === "SetFilter"
    ) {
      if (value.kind === "FunctionConstruction" && value.isRecursive)
        return unsupported("recursive function");
      const bound = bind(
        value.kind === "SetFilter" ? [value.bound] : value.bounds,
        child,
      );
      return {
        op:
          value.kind === "Forall"
            ? "forall"
            : value.kind === "Exists"
              ? "exists"
              : value.kind === "FunctionConstruction"
                ? "function"
                : value.kind === "SetMap"
                  ? "map-set"
                  : "filter",
        bounds: bound.bindings,
        body: expression(
          value.kind === "SetMap"
            ? value.expression
            : value.kind === "SetFilter"
              ? value.predicate
              : value.body,
          bound.context,
        ),
      };
    }
    if (value.kind === "If" && shared) {
      const arms: {
        readonly condition: SafetyExpression;
        readonly consequent: SafetyExpression;
      }[] = [];
      let tail: Expression = value;
      while (tail.kind === "If") {
        arms.push({
          condition: expression(tail.condition, child),
          consequent: expression(tail.consequent, child),
        });
        tail = tail.alternative;
      }
      return arms.reduceRight<SafetyExpression>(
        (alternative, arm) => ({
          op: "if",
          condition: arm.condition,
          consequent: arm.consequent,
          alternative,
        }),
        expression(tail, child),
      );
    }
    if (value.kind === "If")
      return {
        op: "if",
        condition: expression(value.condition, child),
        consequent: expression(value.consequent, child),
        alternative: expression(value.alternative, child),
      };
    if (value.kind === "Case" && (value.other || value.arms.length))
      return value.arms.reduceRight<SafetyExpression>(
        (alternative, arm) => ({
          op: "if",
          condition: expression(arm.condition, child),
          consequent: expression(arm.body, child),
          alternative,
        }),
        value.other ? expression(value.other, child) : unmatchedCase(value),
      );
    if (value.kind === "And" || value.kind === "Or") {
      if (shared) {
        const layers: (readonly Expression[])[] = [];
        let current: readonly Expression[] = value.operands;
        for (;;) {
          layers.push(current);
          const inner = chained(current[0], value.kind);
          if (!inner) break;
          current = inner;
        }
        const first = current[0];
        let result: SafetyExpression = first
          ? expression(first, child)
          : bool(value.kind === "And");
        for (const layer of layers.reverse())
          for (const operand of layer.slice(1))
            result = {
              op: value.kind === "And" ? "and" : "or",
              left: result,
              right: expression(operand, child),
            };
        return result;
      }
      return value.operands.reduce<SafetyExpression>(
        (left, operand) => ({
          op: value.kind === "And" ? "and" : "or",
          left,
          right: expression(operand, child),
        }),
        bool(value.kind === "And"),
      );
    }
    if (value.kind === "BoxAction" || value.kind === "AngleAction") {
      const subscript = expression(value.subscript, child);
      const unchanged: SafetyExpression = {
        op: "eq",
        left: subscript,
        right: prime(subscript),
      };
      return {
        op: value.kind === "BoxAction" ? "or" : "and",
        left: expression(value.action, child),
        right:
          value.kind === "BoxAction"
            ? unchanged
            : { op: "not", argument: unchanged },
      };
    }
    if (value.kind === "Apply") {
      if (context.scope.entries.has(value.operator)) {
        if (shared) return sharedCall(value.operator, value.args, context);
        const expanded = call(value.operator, value.args, context);
        return expression(expanded.body, expanded.context);
      }
      if (
        module.extends.includes("Apalache") &&
        ["ApaFoldSet", "ApaFoldSeqLeft"].includes(value.operator)
      )
        return fold(
          value.operator === "ApaFoldSeqLeft",
          value.args,
          context,
          child,
        );
      const first = value.args[0];
      const second = value.args[1];
      if (
        shared &&
        value.operator === "<=>" &&
        first &&
        second &&
        value.args.length === 2
      )
        return {
          op: "eq",
          left: { op: "not", argument: expression(first, child) },
          right: { op: "not", argument: expression(second, child) },
        };
      if (
        value.operator === "-." ||
        (value.operator === "-" && value.args.length === 1)
      ) {
        if (!module.extends.includes("Integers"))
          return unsupported("unary minus requires Integers");
        if (first?.kind === "Number" && first.value === 2147483648)
          return literal({ kind: "Number", value: -2147483648 });
      }
      if (
        ["~", "\\lnot", "-.", "-"].includes(value.operator) &&
        first &&
        value.args.length === 1
      )
        return {
          op: ["~", "\\lnot"].includes(value.operator) ? "not" : "neg",
          argument: expression(first, child),
        };
      const unary = new Map<
        string,
        | "domain"
        | "cardinality"
        | "len"
        | "head"
        | "tail"
        | "powerset"
        | "union"
      >([
        ["DOMAIN", "domain"],
        ["Cardinality", "cardinality"],
        ["Len", "len"],
        ["Head", "head"],
        ["Tail", "tail"],
        ["SUBSET", "powerset"],
        ["UNION", "union"],
      ]);
      const unaryOp = unary.get(value.operator);
      if (unaryOp && first && value.args.length === 1) {
        if (
          ["len", "head", "tail"].includes(unaryOp) &&
          !module.extends.includes("Sequences")
        )
          return unsupported("sequence operator requires Sequences");
        if (unaryOp === "cardinality" && !module.extends.includes("FiniteSets"))
          return unsupported("Cardinality requires FiniteSets");
        return { op: unaryOp, argument: expression(first, child) };
      }
      if (["\\in", "\\notin"].includes(value.operator) && first && second) {
        const membership = getMembership(
          expression(first, child),
          second,
          child,
        );
        return value.operator === "\\notin"
          ? { op: "not", argument: membership }
          : membership;
      }
      if (value.operator === "..") {
        const slot = context.bound;
        return {
          op: "map-set",
          bounds: [{ slot, domain: domain(value, child, true) }],
          body: { op: "bound", slot },
        };
      }
      if (
        value.operator === "Append" &&
        first &&
        second &&
        module.extends.includes("Sequences")
      )
        return {
          op: "append",
          left: expression(first, child),
          right: expression(second, child),
        };
      if (
        value.operator === "SubSeq" &&
        first &&
        second &&
        value.args[2] &&
        module.extends.includes("Sequences")
      )
        return {
          op: "subseq",
          sequence: expression(first, child),
          from: expression(second, child),
          to: expression(value.args[2], child),
        };
      const op = binary.get(value.operator);
      if (
        op &&
        ["add", "sub", "mul", "lt", "le", "gt", "ge"].includes(op) &&
        !hasArithmetic
      )
        return unsupported("arithmetic requires Naturals");
      if (op === "concat" && !module.extends.includes("Sequences"))
        return unsupported("concatenation requires Sequences");
      if (op && first && second && value.args.length === 2)
        return {
          op,
          left: expression(first, child),
          right: expression(second, child),
        };
    }
    return unsupported(
      `expression ${value.kind}${value.kind === "Apply" ? ` ${value.operator}` : ""}`,
    );
  };
  const membershipSet = (
    value: Expression,
    context: Context,
  ): SafetyExpression => {
    if (shared && value.kind === "Let")
      return membershipSet(value.body, letContext(value, context));
    if (shared && value.kind === "Label")
      return membershipSet(value.body, nested(context));
    if (shared && value.kind === "If") {
      const arms: {
        readonly condition: SafetyExpression;
        readonly consequent: SafetyExpression;
      }[] = [];
      let tail: Expression = value;
      while (tail.kind === "If") {
        arms.push({
          condition: expression(tail.condition, nested(context)),
          consequent: membershipSet(tail.consequent, nested(context)),
        });
        tail = tail.alternative;
      }
      return arms.reduceRight<SafetyExpression>(
        (alternative, arm) => ({
          op: "if",
          condition: arm.condition,
          consequent: arm.consequent,
          alternative,
        }),
        membershipSet(tail, nested(context)),
      );
    }
    if (shared && value.kind === "Case" && (value.other || value.arms.length))
      return value.arms.reduceRight<SafetyExpression>(
        (alternative, arm) => ({
          op: "if",
          condition: expression(arm.condition, nested(context)),
          consequent: membershipSet(arm.body, nested(context)),
          alternative,
        }),
        value.other
          ? membershipSet(value.other, nested(context))
          : unmatchedCase(value),
      );
    if (
      shared &&
      value.kind === "Identifier" &&
      ["Nat", "Int"].includes(value.name)
    ) {
      if (context.bound >= 40)
        return unsupported("more than 40 bound variables");
      return {
        op: "map-set",
        bounds: [{ slot: context.bound, domain: domain(value, context) }],
        body: { op: "bound", slot: context.bound },
      };
    }

    if (
      shared &&
      ((value.kind === "Identifier" &&
        context.scope.entries.has(value.name) &&
        !context.locals.has(value.name)) ||
        (value.kind === "Apply" && context.scope.entries.has(value.operator)))
    )
      return sharedCall(
        value.kind === "Identifier" ? value.name : value.operator,
        value.kind === "Apply" ? value.args : [],
        context,
        "membership-type",
      );
    if (
      value.kind === "Identifier" &&
      context.scope.entries.has(value.name) &&
      !context.locals.has(value.name)
    ) {
      const expanded = call(value.name, [], context);
      return membershipSet(expanded.body, expanded.context);
    }
    if (
      value.kind === "Apply" &&
      value.operator === "Seq" &&
      value.args[0] &&
      module.extends.includes("Sequences")
    )
      return {
        op: "sequence-set",
        argument: membershipSet(value.args[0], nested(context)),
      };
    if (value.kind === "FunctionSet")
      return {
        op: "function-set",
        left: expression(value.domain, nested(context)),
        right: membershipSet(value.range, nested(context)),
      };
    if (value.kind === "RecordSet")
      return {
        op: "record-set",
        fields: value.fields.map((field) => [
          field.name,
          membershipSet(field.value, nested(context)),
        ]),
      };
    return expression(value, context);
  };
  const getMembership = (
    left: SafetyExpression,
    value: Expression,
    context: Context,
  ): SafetyExpression => {
    if (shared && value.kind === "Identifier" && context.locals.has(value.name))
      return { op: "function-in", left, right: membershipSet(value, context) };
    if (
      shared &&
      ((value.kind === "Identifier" &&
        context.scope.entries.has(value.name) &&
        !context.locals.has(value.name)) ||
        (value.kind === "Apply" && context.scope.entries.has(value.operator)))
    )
      return { op: "function-in", left, right: membershipSet(value, context) };
    if (
      value.kind === "Identifier" &&
      context.scope.entries.has(value.name) &&
      !context.locals.has(value.name)
    ) {
      const expanded = call(value.name, [], context);
      return getMembership(left, expanded.body, expanded.context);
    }
    if (
      value.kind === "Apply" &&
      value.operator === "Seq" &&
      value.args[0] &&
      module.extends.includes("Sequences")
    )
      return {
        op: "sequence-in",
        left,
        right: membershipSet(value.args[0], nested(context)),
      };
    if (value.kind === "RecordSet" || value.kind === "FunctionSet")
      return {
        op: "function-in",
        left,
        right: membershipSet(value, nested(context)),
      };
    if (value.kind === "Apply" && value.operator === "SUBSET" && value.args[0])
      return {
        op: "function-in",
        left,
        right: { op: "powerset", argument: expression(value.args[0], context) },
      };
    return { op: "in", value: left, domain: domain(value, context) };
  };
  const target = (value: Expression, context: Context): number | undefined => {
    if (
      context.phase === "initial" &&
      value.kind === "Identifier" &&
      !context.locals.has(value.name)
    )
      return slots.get(value.name);
    if (context.phase === "next" && value.kind === "Prime") {
      const argument = shared
        ? sourceChild(value, "prime-argument")
        : value.expression;
      if (argument.kind === "Identifier" && !context.locals.has(argument.name))
        return slots.get(argument.name);
    }
    return undefined;
  };
  const limited = (
    branches: readonly SafetyBranch[],
  ): readonly SafetyBranch[] => {
    const calls = new Map<
      number,
      { readonly branches: number; readonly steps: number }
    >();
    const size = (
      branches: readonly SafetyBranch[],
    ): { readonly branches: number; readonly steps: number } => {
      let count = 0;
      let maximum = 0;
      for (const branch of branches) {
        let paths = 1;
        const children = shared
          ? analysisChildren({ space: "ir-branch", value: branch }).flatMap(
              (edge) =>
                edge.role === "action-step" && edge.node.space === "ir-step"
                  ? [edge.node.value]
                  : [],
            )
          : branch.steps;
        let steps = children.length;
        for (const step of children) {
          const bodies = shared
            ? analysisChildren({ space: "ir-step", value: step }).flatMap(
                (edge) =>
                  edge.role === "action-frame-body" &&
                  edge.node.space === "ir-branch"
                    ? [edge.node.value]
                    : [],
              )
            : step.kind === "frame"
              ? step.branches
              : [];
          if (step.kind === "call") {
            const definition = definitions[step.definition]?.definition;
            if (definition?.bodyKind === "action") {
              let inner = calls.get(step.definition);
              if (!inner) {
                inner = size(definition.body);
                calls.set(step.definition, inner);
              }
              paths = Math.min(4097, paths * inner.branches);
              steps = Math.min(257, steps + inner.steps);
            }
          }
          if (step.kind === "frame") {
            const inner = size(bodies);
            paths *= inner.branches;
            steps += inner.steps;
          }
        }
        count += paths;
        maximum = Math.max(maximum, steps);
      }
      return { branches: count, steps: maximum };
    };
    const expanded = size(branches);
    if (expanded.branches > 4096 || expanded.steps > 256)
      return unsupported("action expansion exceeds 4096 branches or 256 steps");
    return branches;
  };
  let targetNesting = 0;
  const variableReads = (
    value: SafetyExpression,
    message: string,
    depth = 0,
  ): readonly SafetyExpression[] => {
    if (!shared) return [value];
    if (targetNesting >= 40)
      return unsupported("expression or operator nesting exceeds 40");
    targetNesting++;
    lowerPending();
    const frames: {
      readonly definition?: number;
      readonly arguments: readonly SafetyExpression[];
      readonly captures: readonly SafetyExpression[];
      readonly parent?: number | undefined;
      readonly depth: number;
    }[] = [];
    const pending: {
      readonly value: SafetyExpression;
      readonly frame?: number | undefined;
      readonly depth: number;
    }[] = [{ value, depth }];
    const reads: SafetyExpression[] = [];
    let visited = 0;
    while (pending.length) {
      if (visited++ === targetExpansionLimit)
        unsupported(`target expansion exceeds ${targetExpansionLimit} nodes`);
      const task = pending.pop();
      if (!task) return unsupported(message);
      const { value, frame, depth } = task;
      if (depth >= 40)
        return unsupported("expression or operator nesting exceeds 40");
      const children = analysisChildren({ space: "ir-expression", value });
      if (value.op === "tuple") {
        for (const edge of [...children].reverse())
          if (edge.node.space === "ir-expression")
            pending.push({ value: edge.node.value, frame, depth: depth + 1 });
      } else if (value.op === "read" && !value.next) reads.push(value);
      else if (value.op === "call") {
        let ancestor = frame;
        while (ancestor !== undefined) {
          const owner = frames[ancestor] ?? unsupported(message);
          if (owner.definition === value.definition)
            return unsupported("recursive operator definitions");
          ancestor = owner.parent;
        }
        const entry = definitions[value.definition] ?? unsupported(message);
        lowerDefinition(entry);
        if (entry.definition.bodyKind !== "expression")
          return unsupported(message);
        const body = entry.definition.body;
        const next = frames.length;
        frames.push({
          definition: value.definition,
          arguments: children.flatMap((edge) =>
            edge.role === "call-actual" && edge.node.space === "ir-expression"
              ? [edge.node.value]
              : [],
          ),
          captures: children.flatMap((edge) =>
            edge.role === "call-capture" && edge.node.space === "ir-expression"
              ? [edge.node.value]
              : [],
          ),
          parent: frame,
          depth,
        });
        pending.push({ value: body, frame: next, depth: depth + 1 });
      } else if (value.op === "frame") {
        const next = frames.length;
        frames.push({
          arguments: children.flatMap((edge) =>
            edge.role === "frame-actual" && edge.node.space === "ir-expression"
              ? [edge.node.value]
              : [],
          ),
          captures: children.flatMap((edge) =>
            edge.role === "frame-capture" && edge.node.space === "ir-expression"
              ? [edge.node.value]
              : [],
          ),
          parent: frame,
          depth,
        });
        const body = children.find(
          (edge) =>
            edge.role === "frame-body" && edge.node.space === "ir-expression",
        );
        if (!body || body.node.space !== "ir-expression")
          return unsupported(message);
        pending.push({ value: body.node.value, frame: next, depth: depth + 1 });
      } else if (value.op === "parameter" || value.op === "capture") {
        const caller = frame === undefined ? undefined : frames[frame];
        const actual =
          caller?.[value.op === "parameter" ? "arguments" : "captures"][
            value.slot
          ];
        if (!actual) return unsupported(message);
        pending.push({
          value: actual,
          frame: caller?.parent,
          depth: (caller?.depth ?? depth) + 1,
        });
      } else return unsupported(message);
    }
    targetNesting--;
    return reads;
  };
  const unchangedSteps = (value: SafetyExpression): readonly SafetyStep[] => {
    const children = shared
      ? analysisChildren({ space: "ir-expression", value })
      : [];
    if (value.op === "tuple")
      return shared
        ? children.flatMap((edge) =>
            edge.node.space === "ir-expression"
              ? unchangedSteps(edge.node.value)
              : [],
          )
        : value.values.flatMap(unchangedSteps);
    if (value.op !== "read" || value.next)
      return unsupported("generative UNCHANGED requires state variables");
    return [{ kind: "assign", slot: value.slot, value }];
  };
  const generativeCache = new Map<string, boolean>();
  const generativeCall = (
    name: string,
    context: Context,
    activeCalls: ReadonlySet<Definition>,
  ): boolean => {
    type Task =
      | {
          readonly kind: "call";
          readonly name: string;
          readonly context: Context;
        }
      | {
          readonly kind: "expression";
          readonly value: Expression;
          readonly context: Context;
        }
      | {
          readonly kind: "finish";
          readonly count: number;
          readonly key?: string;
          readonly declaration?: Definition;
        };
    const tasks: Task[] = [{ kind: "call", name, context }];
    const results: boolean[] = [];
    const active = new Set(activeCalls);
    while (tasks.length) {
      const task = tasks.pop();
      if (!task) return unsupported("operator action metadata");
      if (task.kind === "finish") {
        const result = results
          .splice(results.length - task.count)
          .some(Boolean);
        if (task.key !== undefined) generativeCache.set(task.key, result);
        if (task.declaration) active.delete(task.declaration);
        results.push(result);
        continue;
      }
      if (task.kind === "call") {
        const entry = task.context.scope.entries.get(task.name);
        if (
          !entry ||
          entry.definition.isRecursive ||
          active.has(entry.definition)
        )
          return unsupported(`operator ${task.name}`);
        const key = `${identity(entry.definition)}:${task.context.phase}`;
        const found = generativeCache.get(key);
        if (found !== undefined) {
          results.push(found);
          continue;
        }
        if (entry.definition.kind !== "Operator") {
          generativeCache.set(key, false);
          results.push(false);
          continue;
        }
        active.add(entry.definition);
        const locals = new Map(entry.locals);
        for (const parameter of entry.definition.params)
          locals.set(parameter.name, bool(false));
        tasks.push({
          kind: "finish",
          count: 1,
          key,
          declaration: entry.definition,
        });
        tasks.push({
          kind: "expression",
          value: entry.definition.body,
          context: { ...task.context, locals, scope: entry.scope },
        });
        continue;
      }
      const { value, context } = task;
      if (
        value.kind === "Identifier" &&
        context.scope.entries.has(value.name) &&
        !context.locals.has(value.name)
      ) {
        tasks.push({ kind: "call", name: value.name, context });
        continue;
      }
      if (value.kind === "Apply" && context.scope.entries.has(value.operator)) {
        tasks.push({ kind: "call", name: value.operator, context });
        continue;
      }
      if (value.kind === "Apply" && value.args.length >= 2) {
        const args = sourceEdges(value).map((edge) => edge.value);
        const left = args[0];
        const right = args[1];
        const equality =
          value.operator === "=" ||
          (value.operator === ":=" && module.extends.includes("Apalache"));
        results.push(
          ((equality || value.operator === "\\in") &&
            left !== undefined &&
            target(left, context) !== undefined) ||
            (equality &&
              right !== undefined &&
              target(right, context) !== undefined),
        );
        continue;
      }
      if (
        shared &&
        context.phase === "next" &&
        (value.kind === "BoxAction" || value.kind === "AngleAction")
      ) {
        results.push(true);
        continue;
      }
      if (value.kind === "Unchanged" && context.phase === "next") {
        results.push(true);
        continue;
      }
      if (value.kind === "Let") {
        tasks.push({
          kind: "expression",
          value: sourceChild(value, "result"),
          context: letContext(value, context),
        });
        continue;
      }
      if (value.kind === "Exists") {
        const locals = new Map(context.locals);
        for (const bound of value.bounds)
          for (const name of bound.pattern.kind === "Name"
            ? [bound.pattern.name]
            : bound.pattern.names)
            locals.set(name, bool(false));
        const edge = sourceEdges(value).find((edge) => edge.scope === "binder");
        if (!edge) return unsupported("operator action metadata");
        tasks.push({
          kind: "expression",
          value: edge.value,
          context: { ...context, locals },
        });
        continue;
      }
      const children = sourceEdges(value)
        .filter((edge) => edge.forwarding.action === "inherit")
        .map((edge) => edge.value);
      if (!children.length) {
        results.push(false);
        continue;
      }
      tasks.push({ kind: "finish", count: children.length });
      for (let index = children.length - 1; index >= 0; index--) {
        const child = children[index];
        if (child) tasks.push({ kind: "expression", value: child, context });
      }
    }
    return results[0] ?? false;
  };
  interface ChainFrame {
    readonly and: boolean;
    readonly operands: readonly Expression[];
    readonly bound: number;
    position: number;
    result: readonly SafetyBranch[];
    next: SafetyBranch[];
    branch: number;
  }
  type ChainStep =
    | { readonly kind: "descend"; readonly frame: ChainFrame }
    | { readonly kind: "done"; readonly branches: readonly SafetyBranch[] };
  const actionChain = (
    value: Extract<Expression, { kind: "And" | "Or" }>,
    context: Context,
    child: Context,
    name: string,
  ): readonly SafetyBranch[] => {
    const enter = (
      and: boolean,
      operands: readonly Expression[],
      bound: number,
    ): ChainFrame => ({
      and,
      operands,
      bound,
      position: 0,
      result: and ? [{ name, steps: [] }] : [],
      next: [],
      branch: 0,
    });
    const absorb = (frame: ChainFrame, additions: readonly SafetyBranch[]) => {
      const prefix = frame.result[frame.branch];
      if (!prefix) return;
      for (const addition of additions)
        frame.next.push({
          name: addition.name,
          steps: [...prefix.steps, ...addition.steps],
        });
    };
    const accept = (frame: ChainFrame, branches: readonly SafetyBranch[]) => {
      if (frame.and) {
        absorb(frame, branches);
        frame.branch += 1;
      } else for (const branch of branches) frame.next.push(branch);
    };
    const advance = (frame: ChainFrame): ChainStep => {
      if (!frame.and) {
        for (;;) {
          const operand = frame.operands[frame.position];
          if (!operand) return { kind: "done", branches: limited(frame.next) };
          frame.position += 1;
          const inner = chained(operand, "Or");
          if (inner) return { kind: "descend", frame: enter(false, inner, 0) };
          for (const branch of action(operand, child, name))
            frame.next.push(branch);
        }
      }
      for (;;) {
        const operand = frame.operands[frame.position];
        if (!operand) return { kind: "done", branches: frame.result };
        if (frame.branch === frame.result.length) {
          frame.result = limited(frame.next);
          frame.next = [];
          frame.position += 1;
          frame.branch = 0;
          continue;
        }
        const bound =
          frame.bound +
          (frame.result[frame.branch]?.steps.filter(
            (step) => step.kind === "bind",
          ).length ?? 0);
        const inner = chained(operand, "And");
        if (inner) return { kind: "descend", frame: enter(true, inner, bound) };
        absorb(frame, action(operand, { ...child, bound }, name));
        frame.branch += 1;
      }
    };
    const stack = [enter(value.kind === "And", value.operands, context.bound)];
    let finished: readonly SafetyBranch[] | undefined;
    for (let frame = stack.at(-1); frame; frame = stack.at(-1)) {
      if (finished) {
        accept(frame, finished);
        finished = undefined;
      }
      const step = advance(frame);
      if (step.kind === "descend") stack.push(step.frame);
      else {
        stack.pop();
        finished = step.branches;
      }
    }
    return finished ?? [];
  };
  const actionElseIf = (
    value: Extract<Expression, { kind: "If" }>,
    child: Context,
    name: string,
  ): readonly SafetyBranch[] => {
    const arms: {
      readonly condition: SafetyExpression;
      readonly taken: readonly SafetyBranch[];
    }[] = [];
    let tail: Expression = value;
    while (tail.kind === "If") {
      const condition = expression(tail.condition, child);
      arms.push({
        condition,
        taken: action(tail.consequent, child, name).map((branch) => ({
          ...branch,
          steps: [
            { kind: "guard", expression: condition } satisfies SafetyStep,
            ...branch.steps,
          ],
        })),
      });
      tail = tail.alternative;
    }
    return arms.reduceRight<readonly SafetyBranch[]>(
      (alternative, arm) =>
        limited([
          ...arm.taken,
          ...alternative.map((branch) => ({
            ...branch,
            steps: [
              {
                kind: "guard",
                expression: { op: "not", argument: arm.condition },
              } satisfies SafetyStep,
              ...branch.steps,
            ],
          })),
        ]),
      action(tail, child, name),
    );
  };
  const action = (
    value: Expression,
    context: Context,
    name: string,
  ): readonly SafetyBranch[] => {
    const child = nested(context);
    if (
      shared &&
      context.phase === "next" &&
      (value.kind === "BoxAction" || value.kind === "AngleAction")
    ) {
      const subscript = targetExpression(resolveTarget(value.subscript, child));
      const branches = action(value.action, child, name);
      return value.kind === "BoxAction"
        ? limited([...branches, { name, steps: unchangedSteps(subscript) }])
        : limited(
            branches.map((branch) => ({
              ...branch,
              steps: [
                ...branch.steps,
                {
                  kind: "guard",
                  expression: {
                    op: "ne",
                    left: subscript,
                    right: prime(subscript),
                  },
                } satisfies SafetyStep,
              ],
            })),
          );
    }
    if (
      (value.kind === "Identifier" && context.scope.entries.has(value.name)) ||
      (value.kind === "Apply" && context.scope.entries.has(value.operator))
    ) {
      const name = value.kind === "Identifier" ? value.name : value.operator;
      if (shared && !generativeCall(name, context, new Set()))
        return [
          {
            name,
            steps: [
              {
                kind: "guard",
                expression: sharedCall(
                  name,
                  value.kind === "Apply" ? value.args : [],
                  context,
                ),
              },
            ],
          },
        ];
      const entry =
        context.scope.entries.get(name) ?? unsupported(`operator ${name}`);
      if (
        shared &&
        (context.bound > 0 ||
          entry.projection.size > 0 ||
          (entry.definition.kind === "Operator" &&
            entry.definition.params.length > 0))
      ) {
        const call = sharedCall(
          name,
          value.kind === "Apply" ? value.args : [],
          context,
          "value",
          undefined,
          "action",
        );
        if (call.op !== "call") return unsupported("action call metadata");
        return [
          {
            name,
            steps: [
              {
                kind: "call",
                definition: call.definition,
                arguments: call.arguments,
                captures: call.captures,
              },
            ],
          },
        ];
      }
      const expanded = call(
        name,
        value.kind === "Apply" ? value.args : [],
        context,
      );
      const branches = action(expanded.body, expanded.context, name);
      return expanded.frame
        ? limited(
            compactAnonymous([
              {
                name,
                steps: [
                  {
                    kind: "frame",
                    ...expanded.frame,
                    localScope: localScopeOf(branches),
                    branches,
                  },
                ],
              },
            ]),
          )
        : branches;
    }
    if (value.kind === "Let")
      return action(value.body, letContext(value, context), name);
    if (value.kind === "Label") return action(value.body, child, name);
    if (value.kind === "Exists") {
      const bound = bind(value.bounds, child);
      return limited(
        action(value.body, bound.context, name).map((branch) => ({
          ...branch,
          steps: [
            ...bound.bindings.map(
              (binding) => ({ kind: "bind", ...binding }) satisfies SafetyStep,
            ),
            ...branch.steps,
          ],
        })),
      );
    }
    if (shared && (value.kind === "Or" || value.kind === "And"))
      return actionChain(value, context, child, name);
    if (shared && value.kind === "If") return actionElseIf(value, child, name);
    if (value.kind === "Or")
      return limited(
        value.operands.flatMap((operand) => action(operand, child, name)),
      );
    if (value.kind === "And")
      return value.operands.reduce<readonly SafetyBranch[]>(
        (branches, operand) =>
          limited(
            branches.flatMap((branch) =>
              action(
                operand,
                {
                  ...child,
                  bound:
                    context.bound +
                    branch.steps.filter((step) => step.kind === "bind").length,
                },
                name,
              ).map((addition) => ({
                name: addition.name,
                steps: [...branch.steps, ...addition.steps],
              })),
            ),
          ),
        [{ name, steps: [] }],
      );
    if (value.kind === "If") {
      const condition = expression(value.condition, child);
      return limited([
        ...action(value.consequent, child, name).map((branch) => ({
          ...branch,
          steps: [
            { kind: "guard", expression: condition } satisfies SafetyStep,
            ...branch.steps,
          ],
        })),
        ...action(value.alternative, child, name).map((branch) => ({
          ...branch,
          steps: [
            {
              kind: "guard",
              expression: { op: "not", argument: condition },
            } satisfies SafetyStep,
            ...branch.steps,
          ],
        })),
      ]);
    }
    if (value.kind === "Case" && value.arms.length) {
      let branches: readonly SafetyBranch[] = [];
      const previous: SafetyExpression[] = [];
      for (const arm of value.arms) {
        const condition = expression(arm.condition, child);
        const guards: SafetyStep[] = [
          ...previous.map(
            (earlier) =>
              ({
                kind: "guard",
                expression: { op: "not", argument: earlier },
              }) satisfies SafetyStep,
          ),
          { kind: "guard", expression: condition },
        ];
        branches = limited([
          ...branches,
          ...action(arm.body, child, name).map((branch) => ({
            ...branch,
            steps: [...guards, ...branch.steps],
          })),
        ]);
        previous.push(condition);
      }
      const guards: SafetyStep[] = previous.map((earlier) => ({
        kind: "guard",
        expression: { op: "not", argument: earlier },
      }));
      return limited([
        ...branches,
        ...(value.other
          ? action(value.other, child, name).map((branch) => ({
              ...branch,
              steps: [...guards, ...branch.steps],
            }))
          : [
              {
                name,
                steps: [
                  ...guards,
                  {
                    kind: "guard",
                    expression: unmatchedCase(value),
                  } satisfies SafetyStep,
                ],
              },
            ]),
      ]);
    }
    if (value.kind === "Unchanged" && context.phase === "next")
      return [
        {
          name,
          steps: shared
            ? unchangedSteps(
                targetExpression(resolveTarget(value.expression, child, 256)),
              )
            : variableReads(
                expression(value.expression, child),
                targetMessage,
              ).flatMap(unchangedSteps),
        },
      ];
    if (
      value.kind === "Apply" &&
      value.args.length === 2 &&
      value.args[0] &&
      value.args[1]
    ) {
      const slot = target(value.args[0], context);
      if (slot !== undefined && value.operator === "=")
        return [
          {
            name,
            steps: [
              { kind: "assign", slot, value: expression(value.args[1], child) },
            ],
          },
        ];
      if (slot !== undefined && value.operator === "\\in")
        return [
          {
            name,
            steps: [
              {
                kind: "choose",
                slot,
                domain: domain(value.args[1], child, true),
              },
            ],
          },
        ];
      const reverse = target(value.args[1], context);
      if (reverse !== undefined && value.operator === "=")
        return [
          {
            name,
            steps: [
              {
                kind: "assign",
                slot: reverse,
                value: expression(value.args[0], child),
              },
            ],
          },
        ];
    }
    return [
      {
        name,
        steps: [{ kind: "guard", expression: expression(value, context) }],
      },
    ];
  };
  const root = (phase: Context["phase"]): Context => ({
    staticTargets: new Map(),
    targetActuals: new Map(),
    projection: new Map(),
    bindings: new Map(),
    phase,
    locals: new Map(),
    calls: [],
    depth: 0,
    bound: 0,
    scope,
  });
  const operator = (name: string, phase: Context["phase"]) =>
    call(name, [], root(phase));
  const atom = (expression: SafetyExpression): TemporalFormula => ({
    kind: "atom",
    expression,
  });
  const negate = (formula: TemporalFormula): TemporalFormula => {
    const body = formulaAtom(formula);
    return body === undefined
      ? { kind: "not", formula }
      : atom({ op: "not", argument: body });
  };
  const combine = (
    kind: "and" | "or",
    left: TemporalFormula,
    right: TemporalFormula,
  ): TemporalFormula => {
    const leftBody = formulaAtom(left);
    const rightBody = leftBody === undefined ? undefined : formulaAtom(right);
    return leftBody !== undefined && rightBody !== undefined
      ? atom({ op: kind, left: leftBody, right: rightBody })
      : { kind, left, right };
  };
  const isBoxedAction = (value: Expression, context: Context): boolean => {
    if (
      shared &&
      value.kind === "Identifier" &&
      context.locals.has(value.name)
    ) {
      const binding = context.bindings.get(value.name);
      const actual =
        binding === undefined ? undefined : context.targetActuals.get(binding);
      return actual ? isBoxedAction(actual.value, actual.context) : false;
    }
    if (
      value.kind === "Identifier" &&
      context.scope.entries.has(value.name) &&
      !context.locals.has(value.name)
    ) {
      const expanded = call(value.name, [], context);
      return isBoxedAction(expanded.body, expanded.context);
    }
    if (value.kind === "Apply" && context.scope.entries.has(value.operator)) {
      const expanded = call(
        value.operator,
        shared ? sourceEdges(value).map((edge) => edge.value) : value.args,
        context,
      );
      return isBoxedAction(expanded.body, expanded.context);
    }
    return (
      value.kind === "BoxAction" &&
      (!shared ||
        sourceEdges(value).some((edge) => edge.role === "action-body"))
    );
  };
  const temporal = (
    value: Expression,
    context: Context,
    frames: readonly AnonymousFrame[] = [],
  ): TemporalFormula => {
    const child = nested(context);
    if (
      (value.kind === "Identifier" && context.scope.entries.has(value.name)) ||
      (value.kind === "Apply" && context.scope.entries.has(value.operator))
    ) {
      const expanded = call(
        value.kind === "Identifier" ? value.name : value.operator,
        value.kind === "Apply" ? value.args : [],
        context,
      );
      const formula = temporal(
        expanded.body,
        expanded.context,
        expanded.frame ? [...frames, expanded.frame] : frames,
      );
      if (shared) checkDepth(formula);
      return expanded.frame
        ? compactAnonymous({
            kind: "frame",
            ...expanded.frame,
            localScope: localScopeOf(formula),
            formula,
          } as TemporalFormula)
        : formula;
    }
    if (value.kind === "Let")
      return temporal(value.body, letContext(value, context), frames);
    if (value.kind === "Always" || value.kind === "Eventually") {
      const formula = temporal(value.expression, child, frames);
      const body = formulaAtom(formula);
      if (
        value.kind === "Always" &&
        body !== undefined &&
        level(
          frames.reduceRight(
            (body, frame) => expressionFrame(frame, body),
            body,
          ),
        ) > 1 &&
        !isBoxedAction(value.expression, child)
      )
        unsupported("[] action must have the form [A]_v");
      return {
        kind: value.kind === "Always" ? "always" : "eventually",
        formula,
      };
    }
    if (value.kind === "And" || value.kind === "Or")
      return value.operands.reduce<TemporalFormula>(
        (left, operand) =>
          combine(
            value.kind === "And" ? "and" : "or",
            left,
            temporal(operand, child, frames),
          ),
        atom(bool(value.kind === "And")),
      );
    if (value.kind === "Apply" && value.args[0]) {
      if (["~", "\\lnot"].includes(value.operator))
        return negate(temporal(value.args[0], child, frames));
      if (["=>", "~>"].includes(value.operator) && value.args[1]) {
        const right = temporal(value.args[1], child, frames);
        const implication = combine(
          "or",
          negate(temporal(value.args[0], child, frames)),
          value.operator === "~>"
            ? { kind: "eventually", formula: right }
            : right,
        );
        return value.operator === "~>"
          ? { kind: "always", formula: implication }
          : implication;
      }
    }

    if (value.kind === "WeakFairness" || value.kind === "StrongFairness") {
      const subscript = expression(value.subscript, child);
      const enabled = atom({
        op: "enabled",
        branches: action(value.action, child, "fairness"),
        subscript,
      });
      const disabled: TemporalFormula = { kind: "not", formula: enabled };
      const taken = atom({
        op: "and",
        left: expression(value.action, child),
        right: { op: "ne", left: subscript, right: prime(subscript) },
      });
      return value.kind === "WeakFairness"
        ? {
            kind: "always",
            formula: {
              kind: "eventually",
              formula: { kind: "or", left: disabled, right: taken },
            },
          }
        : {
            kind: "or",
            left: {
              kind: "eventually",
              formula: { kind: "always", formula: disabled },
            },
            right: {
              kind: "always",
              formula: { kind: "eventually", formula: taken },
            },
          };
    }
    return atom(expression(value, context));
  };
  const formulaAtom = (
    formula: TemporalFormula,
  ): SafetyExpression | undefined => {
    const children = shared
      ? analysisChildren({ space: "ir-temporal", value: formula })
      : [];
    if (formula.kind === "atom")
      return shared
        ? (children.find((edge) => edge.node.space === "ir-expression")?.node
            .value as SafetyExpression | undefined)
        : formula.expression;
    if (formula.kind !== "frame") return undefined;
    const nested = shared
      ? (children.find(
          (edge) =>
            edge.role === "frame-body" && edge.node.space === "ir-temporal",
        )?.node.value as TemporalFormula | undefined)
      : formula.formula;
    const body = nested && formulaAtom(nested);
    return body ? expressionFrame(formula, body) : undefined;
  };
  const formulaAlways = (
    formula: TemporalFormula,
  ): SafetyExpression | undefined => {
    const children = shared
      ? analysisChildren({ space: "ir-temporal", value: formula })
      : [];
    const nested = shared
      ? (children.find(
          (edge) =>
            edge.node.space === "ir-temporal" &&
            (edge.role === "frame-body" || edge.role === "temporal-body"),
        )?.node.value as TemporalFormula | undefined)
      : "formula" in formula
        ? formula.formula
        : undefined;
    if (formula.kind === "always") return nested && formulaAtom(nested);
    if (formula.kind !== "frame") return undefined;
    const body = nested && formulaAlways(nested);
    return body ? expressionFrame(formula, body) : undefined;
  };
  const validateTemporalActions = (
    formula: TemporalFormula,
    path: readonly string[] = [],
    direct = formulaAlways(formula) !== undefined,
    frames: readonly AnonymousFrame[] = [],
  ): void => {
    if (shared) {
      const children = analysisChildren({
        space: "ir-temporal",
        value: formula,
      });
      for (const edge of children) {
        if (edge.node.space === "ir-temporal") {
          validateTemporalActions(
            edge.node.value,
            formula.kind === "frame" ||
              formula.kind === "not" ||
              formula.kind === "and" ||
              formula.kind === "or"
              ? path
              : [...path, formula.kind],
            direct,
            formula.kind === "frame" ? [...frames, formula] : frames,
          );
        } else if (
          formula.kind === "atom" &&
          edge.node.space === "ir-expression"
        ) {
          const tail = path.slice(-2).join(",");
          const expression = frames.reduceRight(
            (body, frame) => expressionFrame(frame, body),
            edge.node.value,
          );
          if (
            level(expression) > 1 &&
            !(direct && path.length === 1) &&
            tail !== "always,eventually" &&
            tail !== "eventually,always"
          )
            unsupported("temporal actions require []<>A or <>[]A");
        }
      }
      return;
    }
    if (formula.kind === "frame") {
      validateTemporalActions(formula.formula, path, direct, [
        ...frames,
        formula,
      ]);
    } else if (formula.kind === "atom") {
      const tail = path.slice(-2).join(",");
      const expression = frames.reduceRight(
        (body, frame) => expressionFrame(frame, body),
        formula.expression,
      );
      if (
        level(expression) > 1 &&
        !(direct && path.length === 1) &&
        tail !== "always,eventually" &&
        tail !== "eventually,always"
      )
        unsupported("temporal actions require []<>A or <>[]A");
    } else if (formula.kind === "and" || formula.kind === "or") {
      validateTemporalActions(formula.left, path, direct, frames);
      validateTemporalActions(formula.right, path, direct, frames);
    } else if ("formula" in formula)
      validateTemporalActions(
        formula.formula,
        formula.kind === "not" ? path : [...path, formula.kind],
        direct,
        frames,
      );
  };
  const fairness: TemporalFormula[] = [];
  let initial: readonly SafetyBranch[] | undefined;
  let next: readonly SafetyBranch[] | undefined;
  if (config.specification) {
    const spec = operator(config.specification, "next");
    const parts = (value: Expression, context: Context): void => {
      if (
        value.kind === "Identifier" &&
        context.scope.entries.has(value.name)
      ) {
        const expanded = call(value.name, [], context);
        parts(expanded.body, expanded.context);
        return;
      }
      if (value.kind === "And") {
        if (!shared) {
          for (const operand of value.operands) parts(operand, nested(context));
          return;
        }
        const pending = [{ operands: inherited(value), index: 0 }];
        for (let top = pending.at(-1); top; top = pending.at(-1)) {
          const operand = top.operands[top.index];
          top.index += 1;
          if (!operand) pending.pop();
          else if (operand.kind === "And")
            pending.push({ operands: inherited(operand), index: 0 });
          else parts(operand, nested(context));
        }
        return;
      }
      if (value.kind === "Always" && value.expression.kind === "BoxAction") {
        if (next) unsupported("multiple NEXT conjuncts");
        const boxed = shared
          ? sourceChild(value, "temporal-body")
          : value.expression;
        const subscript = expression(
          shared ? sourceChild(boxed, "subscript") : value.expression.subscript,
          context,
        );
        const reads = (inner: SafetyExpression): number[] => {
          const children = shared
            ? analysisChildren({ space: "ir-expression", value: inner })
            : [];
          return inner.op === "tuple"
            ? shared
              ? children.flatMap((edge) =>
                  edge.node.space === "ir-expression"
                    ? reads(edge.node.value)
                    : [],
                )
              : inner.values.flatMap(reads)
            : inner.op === "read" && !inner.next
              ? [inner.slot]
              : unsupported(
                  "SPECIFICATION subscript must list state variables",
                );
        };
        if (
          new Set(
            variableReads(
              subscript,
              "SPECIFICATION subscript must list state variables",
              context.depth,
            ).flatMap(reads),
          ).size !== variables.length
        )
          unsupported(
            "SPECIFICATION subscript must cover every state variable",
          );
        next = action(
          shared ? sourceChild(boxed, "action-body") : value.expression.action,
          context,
          "Next",
        );
        return;
      }
      if (value.kind === "WeakFairness" || value.kind === "StrongFairness") {
        fairness.push(temporal(value, context));
        return;
      }
      const additions = action(value, { ...context, phase: "initial" }, "Init");
      initial = initial
        ? limited(
            initial.flatMap((branch) =>
              additions.map((addition) => ({
                name: addition.name,
                steps: [...branch.steps, ...addition.steps],
              })),
            ),
          )
        : additions;
    };
    parts(spec.body, spec.context);
  } else {
    const initialOperator = operator(
      config.init ?? unsupported("INIT"),
      "initial",
    );
    const nextOperator = operator(config.next ?? unsupported("NEXT"), "next");
    initial = action(
      initialOperator.body,
      initialOperator.context,
      config.init ?? "Init",
    );
    next = action(
      nextOperator.body,
      nextOperator.context,
      config.next ?? "Next",
    );
  }
  if (!initial || !next)
    return unsupported("initial predicate and NEXT are required");
  if (
    !next.some((branch) =>
      branch.steps.some(
        (step) => step.kind !== "guard" || level(step.expression) > 0,
      ),
    )
  )
    return unsupported("NEXT cannot be constant");
  const invariants: SafetyPredicate[] = config.invariants.map((name) => {
    if (shared)
      return { name, expression: sharedCall(name, [], root("state")) };
    const predicate = operator(name, "state");
    return { name, expression: expression(predicate.body, predicate.context) };
  });
  const initialProperties: SafetyPredicate[] = [];
  const actionProperties: SafetyPredicate[] = [];
  const properties: { name: string; formula: TemporalFormula }[] = [];
  for (const name of config.properties) {
    const predicate = operator(name, "next");
    const formula = temporal(predicate.body, predicate.context);
    if (shared) checkDepth(formula);
    validateTemporalActions(formula);
    const stateExpression = formulaAtom(formula);
    const alwaysExpression = formulaAlways(formula);
    if (stateExpression) {
      if (level(stateExpression) > 1)
        return unsupported("initial PROPERTY must be a state predicate");
      initialProperties.push({ name, expression: stateExpression });
    } else if (alwaysExpression) {
      (level(alwaysExpression) > 1 ? actionProperties : invariants).push({
        name,
        expression: alwaysExpression,
      });
    } else properties.push({ name, formula });
  }
  const program = {
    format: "modelt.safety-program" as const,
    version: shared ? 2 : 1,
    name: module.name,
    variables,
    initial,
    next,
    invariants,
    assumptions: module.assumptions.map((assumption, index) => ({
      name: assumption.name ?? `ASSUME ${index + 1}`,
      expression: expression(assumption.expression, root("constant")),
    })),
    checkDeadlock: config.checkDeadlock,
    ...(initialProperties.length ? { initialProperties } : {}),
    ...(actionProperties.length ? { actionProperties } : {}),
    ...(properties.length ? { properties, fairness } : {}),
  };
  if (!shared) return program as unknown as SafetyProgram;
  lowerPending();
  checkDepth(program);
  const compact = compactCaptures(program);
  limited(compact.initial);
  limited(compact.next);
  for (const entry of definitions)
    if (entry.definition.bodyKind === "action") limited(entry.definition.body);
  const phases = ["constant", "initial", "state", "next"];
  const ordered = [...definitions].sort(
    (left, right) =>
      left.declaration.location.line - right.declaration.location.line ||
      left.declaration.location.column - right.declaration.location.column ||
      phases.indexOf(left.phase) - phases.indexOf(right.phase) ||
      Number(left.purpose === "membership-type") -
        Number(right.purpose === "membership-type") ||
      (() => {
        for (
          let index = 0;
          index < Math.min(left.signature.length, right.signature.length);
          index++
        ) {
          const a = left.signature[index];
          const b = right.signature[index];
          if (a && b) {
            const order = compareTarget(a, b);
            if (order) return order;
          }
        }
        return left.signature.length - right.signature.length;
      })() ||
      Number(left.definition.bodyKind === "action") -
        Number(right.definition.bodyKind === "action"),
  );
  const ids = new Map(ordered.map((entry, id) => [entry.definition.id, id]));
  const result = visit(
    {
      ...compact,
      definitions: ordered.map((entry, id) => ({ ...entry.definition, id })),
    },
    (value) =>
      value.op === "call" || value.kind === "call"
        ? { ...value, definition: ids.get(Number(value.definition)) }
        : value,
  ) as SafetyProgram;
  if (!analysisView(result)) unsupported("compiled program metadata");
  checkDepth(result);
  return result;
};
