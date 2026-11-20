import type { SafetyResult } from "../../../packages/idealis/src/contracts.js";
import type { runTlc } from "../collections-temporal/tlc.js";
import { compareOracle } from "../workspace-oracle/node.js";
export interface Observation {
  readonly result?: SafetyResult;
  readonly error?: {
    readonly phase: "compile" | "check";
    readonly code: string;
    readonly message: string;
  };
}
export interface Comparison {
  readonly states?: boolean;
  readonly generated?: boolean;
  readonly shortestTrace?: boolean;
}
export const classifyTlc = (
  reference: Awaited<ReturnType<typeof runTlc>>,
): string => {
  if (reference.exit === null || !reference.log.includes("TLC2 Version"))
    return "infrastructure-error";
  if (
    reference.status === "error" &&
    /^Cannot find source file for module /m.test(reference.log)
  )
    return "infrastructure-error";
  if (
    /^java\.lang\.(NullPointerException|OutOfMemoryError|NoClassDefFoundError)/m.test(
      reference.log,
    )
  )
    return "infrastructure-error";
  if (
    /Semantic errors|Lexical error|Parsing or semantic analysis failed|Fatal errors/.test(
      reference.log,
    )
  )
    return "input-error";
  if (reference.status === "error")
    return reference.log.includes("Starting...") &&
      reference.log.includes("Error:")
      ? "model-error"
      : "infrastructure-error";
  if (
    ["complete", "empty"].includes(reference.status) &&
    (reference.states === null || reference.generated === null)
  )
    return "infrastructure-error";
  return reference.status;
};
export const differences = (
  ours: Observation,
  reference: Awaited<ReturnType<typeof runTlc>>,
  comparison: Comparison = {},
): readonly string[] => {
  const actual = ours.error
    ? ours.error.phase === "compile"
      ? "unsupported"
      : "model-error"
    : ours.result?.status === "property-violation"
      ? "liveness-violation"
      : ours.result?.status;
  const expected = classifyTlc(reference);
  if (expected === "infrastructure-error") return ["infrastructure-error"];
  if (actual === "unsupported" || actual === "bounded") return [String(actual)];
  if (compareOracle(actual, expected).status !== "matched")
    return [`verdict: ${actual} != ${expected}`];
  if (!ours.result) return [];
  const result: string[] = [];
  if (
    comparison.states !== false &&
    reference.states !== null &&
    compareOracle(ours.result.states, reference.states).status !== "matched"
  )
    result.push(`states: ${ours.result.states} != ${reference.states}`);
  if (
    comparison.generated !== false &&
    reference.generated !== null &&
    compareOracle(ours.result.generated, reference.generated).status !==
      "matched"
  )
    result.push(
      `generated: ${ours.result.generated} != ${reference.generated}`,
    );
  if (
    comparison.shortestTrace !== false &&
    ours.result.violation &&
    ["invariant-violation", "property-violation", "deadlock"].includes(
      ours.result.status,
    )
  ) {
    const length = /violated by the initial state/.test(reference.log)
      ? 1
      : [...reference.log.matchAll(/^State \d+:/gm)].length;
    if (
      compareOracle(ours.result.violation.trace.length, length).status !==
      "matched"
    )
      result.push(
        `trace-length: ${ours.result.violation.trace.length} != ${length}`,
      );
  }
  return result;
};
