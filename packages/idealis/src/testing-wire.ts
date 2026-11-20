import type {
  TestingCase,
  TestingComparisonStatus,
  TestingDifferential,
  TestingEffect,
  TestingGenerator,
  TestingJson,
  TestingObservation,
  TestingPhase,
  TestingResult,
  TestingTrial,
  TestingVerdict,
} from "./contracts.js";
import { isRecord } from "./trace.js";

export class TestingError extends Error {
  override readonly name = "TestingError";
  constructor(
    readonly kind: "protocol" | "resource",
    message: string,
  ) {
    super(message);
  }
}
export const fields = (
  input: unknown,
  keys: readonly string[],
): Record<string, unknown> => {
  if (
    !isRecord(input) ||
    Object.keys(input).length !== keys.length ||
    keys.some((key) => !Object.hasOwn(input, key))
  )
    throw new TestingError("protocol", "Unexpected testing fields");
  return input;
};
const text = (input: unknown): string => {
  if (
    typeof input !== "string" ||
    input.length === 0 ||
    new TextEncoder().encode(input).length > 256 ||
    /[\uD800-\uDFFF]/u.test(input)
  )
    throw new TestingError("protocol", "Invalid testing identifier");
  return input;
};
const count = (input: unknown, positive = false): number => {
  if (
    typeof input !== "number" ||
    !Number.isInteger(input) ||
    input < (positive ? 1 : 0) ||
    input > 0xffff_ffff
  )
    throw new TestingError("protocol", "Invalid testing count");
  return input;
};
export const json = (input: unknown, maximumDepth = 64): TestingJson => {
  const ancestors = new Set<object>();
  const visit = (input: unknown, depth: number): TestingJson => {
    if (depth > maximumDepth)
      throw new TestingError("resource", "Testing JSON is too deeply nested");
    if (input === null || typeof input === "boolean") return input;
    if (typeof input === "string") {
      if (/[\uD800-\uDFFF]/u.test(input))
        throw new TestingError("protocol", "Expected Unicode scalar text");
      return input;
    }
    if (typeof input === "number" && Number.isSafeInteger(input))
      return Object.is(input, -0) ? 0 : input;
    if (
      typeof input !== "object" ||
      ancestors.has(input) ||
      (!Array.isArray(input) &&
        Object.getPrototypeOf(input) !== Object.prototype &&
        Object.getPrototypeOf(input) !== null)
    )
      throw new TestingError(
        "protocol",
        "Expected plain acyclic testing JSON with safe integers",
      );
    ancestors.add(input);
    try {
      return Array.isArray(input)
        ? Object.freeze(Array.from(input, (entry) => visit(entry, depth + 1)))
        : Object.freeze(
            Object.fromEntries(
              Object.entries(input)
                .sort(([left], [right]) =>
                  left < right ? -1 : left > right ? 1 : 0,
                )
                .map(([name, value]) => {
                  if (/[\uD800-\uDFFF]/u.test(name))
                    throw new TestingError(
                      "protocol",
                      "Expected Unicode scalar key",
                    );
                  return [name, visit(value, depth + 1)];
                }),
            ),
          );
    } finally {
      ancestors.delete(input);
    }
  };
  return visit(input, 0);
};
export const generator = (input: unknown): TestingGenerator => {
  const value = fields(input, [
    "id",
    "version",
    "initial",
    "states",
    "minLength",
    "maxLength",
  ]);
  if (
    !Array.isArray(value.initial) ||
    value.initial.length === 0 ||
    !isRecord(value.states) ||
    Object.keys(value.states).length > 1024
  )
    throw new TestingError("protocol", "Invalid testing generator states");
  const initial = value.initial.map((entry: unknown) => text(entry));
  const minLength = count(value.minLength);
  const maxLength = count(value.maxLength);
  if (
    minLength > maxLength ||
    maxLength > 128 ||
    new Set(initial).size !== initial.length
  )
    throw new TestingError("protocol", "Invalid testing generator bounds");
  let edges = 0;
  const states = Object.freeze(
    Object.fromEntries(
      Object.entries(value.states).map(([key, entries]) => {
        text(key);
        if (!Array.isArray(entries))
          throw new TestingError("protocol", "Invalid testing generator edges");
        edges += entries.length;
        if (edges > 10_000)
          throw new TestingError(
            "resource",
            "Testing generator exceeds edge limit",
          );
        return [
          key,
          Object.freeze(
            entries.map((input: unknown) => {
              const edge = fields(input, ["action", "target"]);
              const action = json(edge.action, 32);
              if (
                new TextEncoder().encode(JSON.stringify(action)).length > 8192
              )
                throw new TestingError(
                  "resource",
                  "Testing action exceeds 8 KiB",
                );
              return Object.freeze({ action, target: text(edge.target) });
            }),
          ),
        ];
      }),
    ),
  );
  if (
    initial.some((key) => !Object.hasOwn(states, key)) ||
    Object.values(states).some((entries) =>
      entries.some((edge) => !Object.hasOwn(states, edge.target)),
    )
  )
    throw new TestingError("protocol", "Unknown testing generator state");
  return Object.freeze({
    id: text(value.id),
    version: text(value.version),
    initial: Object.freeze(initial),
    states,
    minLength,
    maxLength,
  });
};
export const verdict = (input: unknown): TestingVerdict => {
  if (!isRecord(input))
    throw new TestingError("protocol", "Expected testing verdict");
  switch (input.status) {
    case "matched":
    case "invalid":
      fields(input, ["status"]);
      return Object.freeze({ status: input.status });
    case "mismatch":
      fields(input, ["status", "signature"]);
      return Object.freeze({
        status: input.status,
        signature: text(input.signature),
      });
    case "unknown":
      fields(input, ["status", "reason"]);
      return Object.freeze({
        status: input.status,
        reason: text(input.reason),
      });
    case "error":
      fields(input, ["status", "message"]);
      return Object.freeze({
        status: input.status,
        message: text(input.message),
      });
    default:
      throw new TestingError("protocol", "Unknown testing verdict");
  }
};
const phase = (input: unknown): TestingPhase => {
  if (
    input !== "generation" &&
    input !== "baseline" &&
    input !== "shrink" &&
    input !== "confirmation" &&
    input !== "replay"
  )
    throw new TestingError("protocol", "Unknown testing phase");
  return input;
};
const testCase = (input: unknown): TestingCase => {
  const value = fields(input, [
    "format",
    "version",
    "rng",
    "generatorId",
    "generatorVersion",
    "seed",
    "index",
    "actions",
  ]);
  if (
    value.format !== "idealis.test-case" ||
    value.version !== 1 ||
    value.rng !== "xorshift32-v1" ||
    !Array.isArray(value.actions) ||
    value.actions.length > 128
  )
    throw new TestingError("protocol", "Invalid testing case");
  return Object.freeze({
    format: value.format,
    version: value.version,
    rng: value.rng,
    generatorId: text(value.generatorId),
    generatorVersion: text(value.generatorVersion),
    seed: count(value.seed, true),
    index: count(value.index),
    actions: Object.freeze(
      value.actions.map((action: unknown) => json(action, 32)),
    ),
  });
};
const trial = (input: unknown): TestingTrial => {
  const value = fields(input, ["id", "phase", "case", "verdict"]);
  return Object.freeze({
    id: count(value.id, true),
    phase: phase(value.phase),
    case: testCase(value.case),
    verdict: verdict(value.verdict),
  });
};
const nullableText = (input: unknown): string | null =>
  input === null ? null : text(input);
const result = (input: unknown): TestingResult => {
  const value = fields(input, [
    "status",
    "reason",
    "seed",
    "attempts",
    "cases",
    "skips",
    "work",
    "shrink",
    "counterexample",
    "signature",
    "trials",
  ]);
  if (
    value.status !== "passed" &&
    value.status !== "failed" &&
    value.status !== "exhausted" &&
    value.status !== "inconclusive" &&
    value.status !== "error" &&
    value.status !== "not-failing"
  )
    throw new TestingError("protocol", "Unknown testing result status");
  if (!Array.isArray(value.trials))
    throw new TestingError("protocol", "Missing testing trials");
  const shrink =
    value.shrink === null ? null : fields(value.shrink, ["status", "checks"]);
  if (
    shrink &&
    ((shrink.status !== "minimal" && shrink.status !== "bounded") ||
      count(shrink.checks, true) > 512 ||
      count(shrink.checks) < 2)
  )
    throw new TestingError("protocol", "Invalid testing shrink summary");
  const counterexample =
    value.counterexample === null ? null : testCase(value.counterexample);
  const signature = nullableText(value.signature);
  if (
    (counterexample === null) !== (signature === null) ||
    (value.status === "failed" && !counterexample) ||
    (value.status === "passed" && (count(value.cases) === 0 || counterexample))
  )
    throw new TestingError("protocol", "Testing result and witness differ");
  return Object.freeze({
    status: value.status,
    reason: nullableText(value.reason),
    seed: count(value.seed, true),
    attempts: count(value.attempts),
    cases: count(value.cases),
    skips: count(value.skips),
    work: count(value.work),
    shrink:
      shrink === null
        ? null
        : Object.freeze({
            status: shrink.status === "minimal" ? "minimal" : "bounded",
            checks: count(shrink.checks),
          }),
    counterexample,
    signature,
    trials: Object.freeze(value.trials.map(trial)),
  });
};
export const effect = (input: unknown): TestingEffect => {
  if (!isRecord(input))
    throw new TestingError("protocol", "Missing testing effect");
  if (input.kind === "done") {
    fields(input, ["kind", "result"]);
    return Object.freeze({ kind: "done", result: result(input.result) });
  }
  fields(input, ["kind", "id", "phase", "case"]);
  if (input.kind !== "evaluate")
    throw new TestingError("protocol", "Unknown testing effect");
  return Object.freeze({
    kind: "evaluate",
    id: count(input.id, true),
    phase: phase(input.phase),
    case: testCase(input.case),
  });
};
const observation = (input: unknown): TestingObservation => {
  if (!isRecord(input))
    throw new TestingError("protocol", "Expected testing observation");
  switch (input.kind) {
    case "allowed":
      fields(input, ["kind", "values"]);
      if (!Array.isArray(input.values))
        throw new TestingError("protocol", "Missing allowed values");
      return Object.freeze({
        kind: "allowed",
        values: Object.freeze(
          input.values.map((value: unknown) => json(value, 32)),
        ),
      });
    case "value":
      fields(input, ["kind", "value"]);
      return Object.freeze({ kind: "value", value: json(input.value, 32) });
    case "invalid":
    case "unknown":
      fields(input, ["kind", "reason"]);
      return Object.freeze({ kind: input.kind, reason: text(input.reason) });
    case "error":
      fields(input, ["kind", "message"]);
      return Object.freeze({ kind: "error", message: text(input.message) });
    default:
      throw new TestingError("protocol", "Unknown testing observation");
  }
};
const status = (input: unknown): TestingComparisonStatus => {
  if (
    input !== "matched" &&
    input !== "mismatch" &&
    input !== "invalid" &&
    input !== "unknown" &&
    input !== "error"
  )
    throw new TestingError("protocol", "Unknown comparison status");
  return input;
};
export const differential = (input: unknown): TestingDifferential => {
  const value = fields(input, ["status", "oracle", "comparisons"]);
  if (!Array.isArray(value.comparisons) || value.comparisons.length === 0)
    throw new TestingError("protocol", "Empty testing comparison");
  return Object.freeze({
    status: status(value.status),
    oracle: observation(value.oracle),
    comparisons: Object.freeze(
      value.comparisons.map((input: unknown) => {
        const value = fields(input, [
          "id",
          "status",
          "expected",
          "actual",
          "reason",
          "outcome",
        ]);
        if (!Array.isArray(value.expected))
          throw new TestingError("protocol", "Missing expected values");
        return Object.freeze({
          id: text(value.id),
          status: status(value.status),
          expected: Object.freeze(
            value.expected.map((input: unknown) => json(input, 32)),
          ),
          actual: json(value.actual, 32),
          reason: nullableText(value.reason),
          outcome: observation(value.outcome),
        });
      }),
    ),
  });
};
export const reply = (
  input: unknown,
  kind: string,
): Record<string, unknown> => {
  if (isRecord(input) && input.kind === "error") {
    const value = fields(fields(input, ["kind", "error"]).error, [
      "kind",
      "message",
    ]);
    if (value.kind !== "protocol" && value.kind !== "resource")
      throw new TestingError("protocol", "Unknown testing error kind");
    throw new TestingError(
      value.kind,
      typeof value.message === "string"
        ? value.message
        : "Invalid testing error message",
    );
  }
  const value = fields(
    input,
    kind === "effect"
      ? ["kind", "effect"]
      : kind === "generator"
        ? ["kind", "generator"]
        : kind === "comparison"
          ? ["kind", "result"]
          : ["kind"],
  );
  if (value.kind !== kind)
    throw new TestingError("protocol", "Unexpected testing response");
  return value;
};
