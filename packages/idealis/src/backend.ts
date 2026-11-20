import type {
  BackendBehaviorWitness,
  BackendDescriptor,
  BackendIdentity,
  BackendOutcome,
  BackendQuery,
  BackendRequest,
  BackendResult,
  BackendScope,
  PortableCore,
  SafetyProgram,
  VerificationBackend,
} from "./contracts.js";
import { normalizePortableValue, PortableCoreError } from "./portable-value.js";
import { checkSafety, decodeSafetyResult } from "./safety.js";
import { isRecord } from "./trace.js";

export type {
  BackendBehaviorWitness,
  BackendDescriptor,
  BackendIdentity,
  BackendOutcome,
  BackendQuery,
  BackendRequest,
  BackendResult,
  BackendScope,
  VerificationBackend,
} from "./contracts.js";

export class BackendContractError extends Error {
  override readonly name = "BackendContractError";
}

const fields = (
  value: unknown,
  required: readonly string[],
  optional: readonly string[] = [],
): Record<string, unknown> => {
  if (
    !isRecord(value) ||
    required.some((key) => !Object.hasOwn(value, key)) ||
    Object.keys(value).some(
      (key) => !required.includes(key) && !optional.includes(key),
    )
  )
    throw new BackendContractError("Invalid backend fields");
  return value;
};
const text = (value: unknown): string => {
  if (
    typeof value !== "string" ||
    value.trim().length === 0 ||
    /[\uD800-\uDFFF]/u.test(value)
  )
    throw new BackendContractError(
      "Expected a nonempty backend identifier or message",
    );
  return value;
};
const count = (value: unknown, positive = false): number => {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < (positive ? 1 : 0) ||
    value > 0xffff_ffff
  )
    throw new BackendContractError("Invalid backend count");
  return value;
};
const reason = (value: unknown) => {
  switch (value) {
    case "depth":
    case "states":
    case "work":
    case "temporal":
    case "solver-unknown":
    case "cancelled":
    case "unsupported":
    case "initialization-unknown":
    case "evaluation":
    case "obligations":
      return value;
    default:
      throw new BackendContractError("Unknown inconclusive reason");
  }
};
const strings = (value: unknown): readonly string[] => {
  if (!Array.isArray(value))
    throw new BackendContractError("Expected backend identifiers");
  const result = value.map(text);
  if (new Set(result).size !== result.length)
    throw new BackendContractError("Duplicate backend identifiers");
  return Object.freeze(result);
};
const identity = (input: unknown): BackendIdentity => {
  const value = fields(input, ["id", "version"]);
  return Object.freeze({ id: text(value.id), version: text(value.version) });
};
const scope = (input: unknown): BackendScope => {
  const value = fields(input, [
    "modelId",
    "propertyId",
    "semantics",
    "assumptions",
  ]);
  return Object.freeze({
    modelId: text(value.modelId),
    propertyId: text(value.propertyId),
    semantics: text(value.semantics),
    assumptions: strings(value.assumptions),
  });
};
const query = (input: unknown): BackendQuery => {
  if (!isRecord(input))
    throw new BackendContractError("Expected backend query");
  switch (input.kind) {
    case "explicit": {
      const value = fields(input, ["kind", "bounds"]);
      const bounds = fields(
        value.bounds,
        ["maxDepth", "maxStates", "maxWork"],
        ["maxTemporalNodes"],
      );
      return Object.freeze({
        kind: "explicit",
        bounds: Object.freeze({
          maxDepth: count(bounds.maxDepth),
          maxStates: count(bounds.maxStates, true),
          maxWork: count(bounds.maxWork, true),
          ...(bounds.maxTemporalNodes === undefined
            ? {}
            : { maxTemporalNodes: count(bounds.maxTemporalNodes, true) }),
        }),
      });
    }
    case "bounded": {
      const value = fields(input, ["kind", "maxDepth"]);
      return Object.freeze({
        kind: "bounded",
        maxDepth: count(value.maxDepth),
      });
    }
    case "induction": {
      const value = fields(input, ["kind", "invariantId", "obligation"]);
      if (
        value.obligation !== "init" &&
        value.obligation !== "step" &&
        value.obligation !== "implication" &&
        value.obligation !== "all"
      )
        throw new BackendContractError("Unknown induction obligation");
      return Object.freeze({
        kind: "induction",
        invariantId: text(value.invariantId),
        obligation: value.obligation,
      });
    }
    default:
      throw new BackendContractError("Unknown backend query");
  }
};
const behavior = (input: unknown): BackendBehaviorWitness => {
  const value = fields(
    input,
    ["kind", "violation", "name", "variables", "trace"],
    ["loopStart", "loopAction"],
  );
  if (
    value.kind !== "behavior" ||
    (value.violation !== "invariant-violation" &&
      value.violation !== "property-violation" &&
      value.violation !== "liveness-violation" &&
      value.violation !== "deadlock")
  )
    throw new BackendContractError("Invalid behavior witness");
  const variables = strings(value.variables);
  if (!variables.length || !Array.isArray(value.trace) || !value.trace.length)
    throw new BackendContractError("Empty behavior witness");
  const trace = value.trace.map((input: unknown, index: number) => {
    const step = fields(input, ["action", "values"]);
    if (
      (index === 0 ? step.action !== null : typeof step.action !== "string") ||
      !Array.isArray(step.values) ||
      step.values.length !== variables.length
    )
      throw new BackendContractError("Invalid behavior state");
    return Object.freeze({
      action: step.action === null ? null : text(step.action),
      values: Object.freeze(
        step.values.map((entry: unknown) => normalizePortableValue(entry)),
      ),
    });
  });
  const liveness = value.violation === "liveness-violation";
  if (
    liveness
      ? value.loopStart === undefined ||
        value.loopAction === undefined ||
        count(value.loopStart) >= trace.length
      : value.loopStart !== undefined || value.loopAction !== undefined
  )
    throw new BackendContractError("Invalid behavior loop");
  return Object.freeze({
    kind: "behavior",
    violation: value.violation,
    name: text(value.name),
    variables,
    trace: Object.freeze(trace),
    ...(liveness
      ? {
          loopStart: count(value.loopStart),
          loopAction: text(value.loopAction),
        }
      : {}),
  });
};
const sameContext = (left: BackendResult, right: BackendResult): boolean =>
  JSON.stringify(left.backend) === JSON.stringify(right.backend) &&
  JSON.stringify(left.scope) === JSON.stringify(right.scope);
const inductionReady = (
  context: BackendResult,
  obligations: readonly BackendResult[],
): boolean => {
  if (
    context.query.kind !== "induction" ||
    !obligations.length ||
    obligations.length > 3
  )
    throw new BackendContractError("Invalid induction evidence");
  const names = new Set<string>();
  for (const entry of obligations) {
    if (
      !sameContext(context, entry) ||
      entry.query.kind !== "induction" ||
      entry.query.obligation === "all" ||
      entry.query.invariantId !== context.query.invariantId ||
      names.has(entry.query.obligation)
    )
      throw new BackendContractError(
        "Induction contexts or obligations differ",
      );
    names.add(entry.query.obligation);
  }
  return (
    names.has("init") &&
    names.has("step") &&
    (context.query.invariantId === context.scope.propertyId ||
      names.has("implication")) &&
    obligations.every((entry) => entry.outcome.kind === "obligation-holds") &&
    obligations.find(
      (entry) =>
        entry.query.kind === "induction" && entry.query.obligation === "init",
    )?.initialization === "nonempty"
  );
};
const parseResult = (input: unknown, nested: boolean): BackendResult => {
  const value = fields(
    input,
    [
      "format",
      "version",
      "backend",
      "scope",
      "query",
      "initialization",
      "outcome",
    ],
    ["progress"],
  );
  if (
    value.format !== "modelt.backend-result" ||
    value.version !== 1 ||
    (value.initialization !== "nonempty" &&
      value.initialization !== "empty" &&
      value.initialization !== "unknown")
  )
    throw new BackendContractError(
      "Unsupported backend result version or initialization",
    );
  const context = {
    format: "modelt.backend-result",
    version: 1,
    backend: identity(value.backend),
    scope: scope(value.scope),
    query: query(value.query),
    initialization: value.initialization,
  } satisfies Omit<BackendResult, "outcome">;
  if (!isRecord(value.outcome))
    throw new BackendContractError("Expected backend outcome");
  const raw = value.outcome;
  const outcome: BackendOutcome = (() => {
    switch (raw.kind) {
      case "explicit-complete": {
        fields(raw, ["kind", "states"]);
        if (
          context.query.kind !== "explicit" ||
          context.initialization !== "nonempty" ||
          count(raw.states, true) > context.query.bounds.maxStates
        )
          throw new BackendContractError("Invalid complete claim");
        return { kind: "explicit-complete", states: count(raw.states, true) };
      }
      case "bounded-no-counterexample": {
        fields(raw, ["kind", "maxDepth"]);
        if (
          context.query.kind !== "bounded" ||
          context.initialization !== "nonempty" ||
          count(raw.maxDepth) !== context.query.maxDepth
        )
          throw new BackendContractError("Invalid bounded claim");
        return {
          kind: "bounded-no-counterexample",
          maxDepth: count(raw.maxDepth),
        };
      }
      case "obligation-holds": {
        fields(raw, ["kind"]);
        if (
          context.query.kind !== "induction" ||
          context.query.obligation === "all"
        )
          throw new BackendContractError(
            "An obligation is not an inductive proof",
          );
        return { kind: "obligation-holds" };
      }
      case "counterexample": {
        fields(raw, ["kind", "witness"]);
        if (context.query.kind === "induction") {
          const witness = fields(raw.witness, ["kind", "assignment"]);
          if (
            context.query.obligation === "all" ||
            witness.kind !== "obligation"
          )
            throw new BackendContractError("Expected an obligation assignment");
          return {
            kind: "counterexample",
            witness: {
              kind: "obligation",
              assignment: normalizePortableValue(witness.assignment),
            },
          };
        }
        if (context.initialization !== "nonempty")
          throw new BackendContractError(
            "A behavior needs a nonempty initial domain",
          );
        const witness = behavior(raw.witness);
        if (
          context.query.kind === "bounded" &&
          witness.trace.length -
            1 +
            (witness.violation === "liveness-violation" ? 1 : 0) >
            context.query.maxDepth
        )
          throw new BackendContractError("Witness exceeds the bounded query");
        return { kind: "counterexample", witness };
      }
      case "empty": {
        fields(raw, ["kind"]);
        if (context.initialization !== "empty")
          throw new BackendContractError(
            "Empty result needs an empty initial domain",
          );
        return { kind: "empty" };
      }
      case "error": {
        fields(raw, ["kind", "code", "message"]);
        return {
          kind: "error",
          code: text(raw.code),
          message: text(raw.message),
        };
      }
      case "inductive":
      case "inconclusive": {
        fields(
          raw,
          raw.kind === "inductive"
            ? ["kind", "obligations"]
            : ["kind", "reason"],
          raw.kind === "inconclusive" ? ["obligations"] : [],
        );
        const obligations =
          raw.obligations === undefined
            ? undefined
            : (() => {
                if (
                  nested ||
                  context.query.kind !== "induction" ||
                  context.query.obligation !== "all" ||
                  !Array.isArray(raw.obligations) ||
                  raw.obligations.length < 1 ||
                  raw.obligations.length > 3
                )
                  throw new BackendContractError("Invalid induction evidence");
                return Object.freeze(
                  raw.obligations.map((entry: unknown) =>
                    parseResult(entry, true),
                  ),
                );
              })();
        if (raw.kind === "inductive") {
          const candidate: BackendResult = {
            ...context,
            outcome: { kind: "inductive", obligations: obligations ?? [] },
          };
          if (
            context.initialization !== "nonempty" ||
            !obligations ||
            !inductionReady(candidate, obligations)
          )
            throw new BackendContractError("Incomplete induction proof");
          return { kind: "inductive", obligations };
        }
        if ((raw.reason === "obligations") !== (obligations !== undefined))
          throw new BackendContractError(
            "Incomplete induction needs its obligations",
          );
        if (obligations)
          inductionReady(
            {
              ...context,
              outcome: { kind: "inconclusive", reason: "obligations" },
            },
            obligations,
          );
        return {
          kind: "inconclusive",
          reason: reason(raw.reason),
          ...(obligations ? { obligations } : {}),
        };
      }
      default:
        throw new BackendContractError("Unknown backend outcome");
    }
  })();
  const progress =
    value.progress === undefined
      ? undefined
      : (() => {
          const progress = fields(value.progress, [
            "states",
            "generated",
            "depth",
            "work",
          ]);
          return Object.freeze({
            states: count(progress.states),
            generated: count(progress.generated),
            depth: count(progress.depth),
            work: count(progress.work),
          });
        })();
  return Object.freeze({
    ...context,
    outcome: Object.freeze(outcome),
    ...(progress ? { progress } : {}),
  });
};

export const parseBackendResult = (input: unknown): BackendResult =>
  parseResult(input, false);

export const completeInduction = (
  input: readonly BackendResult[],
): BackendResult => {
  const obligations = input.map(parseBackendResult);
  const first = obligations[0];
  if (first?.query.kind !== "induction" || first.query.obligation === "all")
    throw new BackendContractError("Expected induction obligations");
  const ready = inductionReady(first, obligations);
  const base = obligations.find(
    (entry) =>
      entry.query.kind === "induction" && entry.query.obligation === "init",
  );
  return parseBackendResult({
    format: first.format,
    version: first.version,
    backend: first.backend,
    scope: first.scope,
    query: { ...first.query, obligation: "all" },
    initialization: base?.initialization ?? "unknown",
    outcome: ready
      ? { kind: "inductive", obligations }
      : { kind: "inconclusive", reason: "obligations", obligations },
  });
};

export const parseBackendRequest = <Input>(
  input: BackendRequest<Input>,
): BackendRequest<Input> => {
  const raw = fields(input, ["format", "version", "scope", "query", "input"]);
  if (raw.format !== "modelt.backend-request" || raw.version !== 1)
    throw new BackendContractError("Unsupported backend request version");
  return Object.freeze({
    format: "modelt.backend-request",
    version: 1,
    scope: scope(raw.scope),
    query: query(raw.query),
    input: structuredClone(input.input),
  });
};

export const runBackend = async <Input>(
  backend: VerificationBackend<Input>,
  input: BackendRequest<Input>,
): Promise<BackendResult> => {
  const request = parseBackendRequest(input);
  const descriptor = fields(backend.descriptor, [
    "id",
    "version",
    "queries",
    "semantics",
  ]);
  const engine = identity({ id: descriptor.id, version: descriptor.version });
  const queries = strings(descriptor.queries);
  if (
    !queries.length ||
    queries.some(
      (name) =>
        name !== "explicit" && name !== "bounded" && name !== "induction",
    )
  )
    throw new BackendContractError("Invalid backend capabilities");
  const semantics = strings(descriptor.semantics);
  if (!semantics.length)
    throw new BackendContractError("Empty backend semantic profiles");
  if (
    !queries.includes(request.query.kind) ||
    !semantics.includes(request.scope.semantics)
  )
    return parseBackendResult({
      format: "modelt.backend-result",
      version: 1,
      backend: engine,
      scope: request.scope,
      query: request.query,
      initialization: "unknown",
      outcome: { kind: "inconclusive", reason: "unsupported" },
    });
  const result = parseBackendResult(await backend.check(request));
  if (
    JSON.stringify(result.backend) !== JSON.stringify(engine) ||
    JSON.stringify(result.scope) !== JSON.stringify(request.scope) ||
    JSON.stringify(result.query) !== JSON.stringify(request.query)
  )
    throw new BackendContractError(
      "Backend replied for a different engine, scope or query",
    );
  return result;
};

export const createRtlcBackend = (
  core: PortableCore,
  options: { readonly version: string },
): VerificationBackend<SafetyProgram> => {
  const descriptor: BackendDescriptor = Object.freeze({
    id: "rtlc",
    version: text(options.version),
    queries: Object.freeze(["explicit"] as const),
    semantics: Object.freeze(["tlc-1.7.4-int32"]),
  });
  return {
    descriptor,
    check: async (request) => {
      const base = {
        format: "modelt.backend-result",
        version: 1,
        backend: { id: descriptor.id, version: descriptor.version },
        scope: request.scope,
        query: request.query,
      } as const;
      if (
        request.query.kind !== "explicit" ||
        !descriptor.semantics.includes(request.scope.semantics)
      )
        return parseBackendResult({
          ...base,
          initialization: "unknown",
          outcome: { kind: "inconclusive", reason: "unsupported" },
        });
      try {
        const result = decodeSafetyResult(
          checkSafety(core, request.input, request.query.bounds),
        );
        const progress = {
          states: result.states,
          generated: result.generated,
          depth: result.depth,
          work: result.work,
        };
        const initialization =
          result.states > 0
            ? "nonempty"
            : result.status === "empty"
              ? "empty"
              : "unknown";
        const outcome: BackendOutcome =
          result.status === "complete"
            ? { kind: "explicit-complete", states: result.states }
            : result.status === "empty"
              ? { kind: "empty" }
              : result.status === "bounded"
                ? { kind: "inconclusive", reason: result.reason ?? "work" }
                : result.status === "assumption-violation"
                  ? {
                      kind: "error",
                      code: "assumption-violation",
                      message: result.violation?.name ?? "Assumption violated",
                    }
                  : result.violation
                    ? {
                        kind: "counterexample",
                        witness: {
                          kind: "behavior",
                          violation: result.status,
                          variables: [...request.input.variables],
                          ...result.violation,
                        },
                      }
                    : {
                        kind: "error",
                        code: "protocol",
                        message: "Missing rtlc counterexample",
                      };
        return parseBackendResult({
          ...base,
          initialization,
          progress,
          outcome,
        });
      } catch (error) {
        if (!(error instanceof PortableCoreError)) throw error;
        return parseBackendResult({
          ...base,
          initialization: "unknown",
          outcome: { kind: "error", code: error.code, message: error.message },
        });
      }
    },
  };
};
