import assert from "node:assert/strict";
import type {
  PortableCore,
  SafetyOptions,
} from "../../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../../packages/idealis/src/portable-value.js";
import {
  checkSafety,
  decodeSafetyResult,
} from "../../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { bounds } from "../collections-temporal/cases.js";
import { runTlc, traceInput } from "../collections-temporal/tlc.js";
import type { startNative } from "../usable/native.js";
import { compareOracle } from "../workspace-oracle/node.js";
import { classifyTlc, differences, type Observation } from "./compare.js";
import type { Input } from "./corpus.js";

export const differential = async (
  input: Input,
  core: PortableCore,
  native: ReturnType<typeof startNative>,
  java: string,
  jar: string,
  directory: string,
  frontend: typeof compileTlaSafety = compileTlaSafety,
  options: NonNullable<Parameters<typeof runTlc>[5]> = {},
  safetyOptions?: SafetyOptions,
) => {
  const observe = (): Observation => {
    try {
      const program = frontend(input);
      try {
        return { result: checkSafety(core, program, bounds, safetyOptions) };
      } catch (error) {
        assert.ok(error instanceof PortableCoreError);
        return {
          error: { phase: "check", code: error.code, message: error.message },
        };
      }
    } catch (error) {
      assert.ok(error instanceof Error);
      return {
        error: {
          phase: "compile",
          code: "unsupported",
          message: error.message,
        },
      };
    }
  };
  const ours = observe();
  let nativeResult: Observation | undefined;
  if (ours.error?.phase !== "compile") {
    const response = await native.request({
      version: 1,
      request: {
        kind: "check-safety",
        program: frontend(input),
        bounds,
        ...(safetyOptions === undefined ? {} : { options: safetyOptions }),
      },
    });
    assert.ok(isRecord(response));
    if (response.kind === "safety")
      nativeResult = { result: decodeSafetyResult(response.result) };
    else {
      assert.equal(response.kind, "error");
      assert.equal(typeof response.code, "string");
      assert.equal(typeof response.message, "string");
      nativeResult = {
        error: {
          phase: "check",
          code: String(response.code),
          message: String(response.message),
        },
      };
    }
  }
  const reference = await runTlc(
    java,
    jar,
    `${directory}/model`,
    input,
    "Differential",
    options,
  );
  const rejected =
    input.expectedRejection === true &&
    ours.error?.phase === "compile" &&
    /TLC 1.7.4 string literal/.test(ours.error.message) &&
    classifyTlc(reference) === "input-error";
  const comparison = safetyOptions?.partialOrder
    ? { states: false, generated: false, shortestTrace: false }
    : safetyOptions?.symmetryActions
      ? { generated: false }
      : {};
  const mismatches = rejected
    ? []
    : [...differences(ours, reference, comparison)];
  if (nativeResult && compareOracle(nativeResult, ours).status !== "matched")
    mismatches.push("native-wasm");
  if (
    input.name === "positive-control" &&
    classifyTlc(reference) !== "complete"
  )
    mismatches.push("positive-control");
  if (
    input.name === "error-control" &&
    classifyTlc(reference) !== "model-error"
  )
    mismatches.push("error-control");
  let witness:
    | {
        readonly input: ReturnType<typeof traceInput>;
        readonly status: string;
        readonly states: number | null;
        readonly tautologyTrace?: {
          readonly status: string;
          readonly states: number | null;
        };
      }
    | undefined;
  if (ours.result?.violation?.trace.length) {
    const model =
      ours.result.status === "deadlock"
        ? {
            ...input,
            source: input.source.replace(
              /=+\s*$/,
              "__HasNext == ENABLED Next\n====",
            ),
          }
        : input;
    const result =
      ours.result.status === "deadlock"
        ? {
            ...ours.result,
            violation: { ...ours.result.violation, name: "__HasNext" },
          }
        : ours.result;
    const forced = traceInput(model, frontend(input), result);
    const replay = await runTlc(
      java,
      jar,
      `${directory}/witness`,
      forced,
      "Differential",
      options,
    );
    witness = {
      input: forced,
      status: classifyTlc(replay),
      states: replay.states,
    };
    if (
      forced.expected === "complete" &&
      classifyTlc(replay) === "model-error" &&
      replay.log.includes(
        "Temporal formula is a tautology (its negation is unsatisfiable)",
      )
    ) {
      const traceLength = result.violation?.trace.length;
      assert.ok(traceLength);
      const reached = await runTlc(
        java,
        jar,
        `${directory}/witness-validity`,
        {
          source: forced.source.replace(
            /=+\s*$/,
            `__TraceReached == <> (__position = ${traceLength - 1})\n====`,
          ),
          config: forced.config.replace(
            "PROPERTY __Expected",
            "PROPERTY __TraceReached",
          ),
        },
        "Differential",
        options,
      );
      witness = {
        ...witness,
        tautologyTrace: {
          status: classifyTlc(reached),
          states: reached.states,
        },
      };
    }
    if (
      (witness.tautologyTrace?.status ?? witness.status) !== forced.expected ||
      (witness.tautologyTrace?.states ?? witness.states) !==
        result.violation?.trace.length
    )
      mismatches.push("witness");
  }
  return {
    input,
    comparison,
    ...(safetyOptions?.symmetryActions
      ? {
          generatedComparison: "not comparable: action orbit representatives",
          safetyOptions,
        }
      : {}),
    classification: rejected ? "rejected-input" : "compared",
    ours,
    ...(nativeResult ? { native: nativeResult } : {}),
    reference: {
      status: classifyTlc(reference),
      states: reference.states,
      generated: reference.generated,
      exit: reference.exit,
    },
    ...(witness ? { witness } : {}),
    differences: mismatches,
  };
};
export type Report = Awaited<ReturnType<typeof differential>>;
