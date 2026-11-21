import assert from "node:assert/strict";
import type {
  SafetyBounds,
  SafetyProgram,
} from "../../../packages/idealis/src/contracts.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";

export const frontendLimit = "expression or operator nesting exceeds 40";
export const checkerLimit = "Safety expression nesting exceeds 48";
export const parserLimit = "TLA parsing: Expression nesting exceeds 64";

export const bounds: SafetyBounds = {
  maxDepth: 100,
  maxStates: 1000,
  maxWork: 1_000_000,
};

export type Outcome =
  | { readonly stage: "compile"; readonly message: string }
  | { readonly stage: "check"; readonly message: string }
  | {
      readonly stage: "result";
      readonly status: "complete";
      readonly states: number;
      readonly generated: number;
      readonly work: number | null;
    }
  | {
      readonly stage: "result";
      readonly status: "invariant-violation";
      readonly trace: number;
      readonly work: number | null;
    };

export interface Input {
  readonly source: string;
  readonly config: string;
  readonly irVersion: 1 | 2;
}

export type Frontend = "ts" | "rust";

export interface Cell {
  readonly v1: Outcome;
  readonly v2: Outcome;
}

export type Expectation = Readonly<Record<Frontend, Cell>>;

export const message = (text: string): string =>
  text.replace(/^Unsupported TLA input: /, "").replace(/ at \d+:\d+$/, "");

export const compileFailure = (text: string): Outcome => ({
  stage: "compile",
  message: message(text),
});

export const checked = (reply: unknown): Outcome => {
  assert.ok(isRecord(reply), "check reply");
  if (reply.kind === "error") {
    assert.equal(typeof reply.message, "string");
    return { stage: "check", message: message(String(reply.message)) };
  }
  assert.equal(reply.kind, "safety");
  const result = reply.result;
  assert.ok(isRecord(result), "safety result");
  if (result.status === "complete")
    return {
      stage: "result",
      status: "complete",
      states: Number(result.states),
      generated: Number(result.generated),
      work: Number(result.work),
    };
  assert.equal(result.status, "invariant-violation");
  const violation = result.violation;
  assert.ok(isRecord(violation) && Array.isArray(violation.trace));
  return {
    stage: "result",
    status: "invariant-violation",
    trace: violation.trace.length,
    work: Number(result.work),
  };
};

export interface Observation {
  readonly outcome: Outcome;
  readonly program: SafetyProgram | null;
}

export const observe = async (
  compile: (input: Input) => Promise<SafetyProgram>,
  check: (program: SafetyProgram) => Promise<unknown>,
  input: Input,
): Promise<Observation> => {
  let program: SafetyProgram;
  try {
    program = await compile(input);
  } catch (error) {
    assert.ok(error instanceof Error);
    return { outcome: compileFailure(error.message), program: null };
  }
  return { outcome: checked(await check(program)), program };
};
