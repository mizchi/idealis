import { isRecord } from "../../../packages/idealis/src/trace.js";
export interface Answer {
  readonly status: "sat" | "unsat" | "unknown";
  readonly values: Readonly<Record<string, string>>;
  readonly booleans: Readonly<Record<string, boolean>>;
  readonly reason?: string;
}
export interface Solution extends Answer {
  readonly stats: {
    readonly work: number;
    readonly branches: number;
    readonly relaxations: number;
  };
}
export interface Assumption {
  readonly symbol: string;
  readonly positive: boolean;
}
export type Request =
  | {
      readonly kind: "run-script";
      readonly source: string;
      readonly maxWork: number;
    }
  | {
      readonly kind: "solve";
      readonly source: string;
      readonly maxWork: number;
    }
  | { readonly kind: "prepare"; readonly source: string }
  | {
      readonly kind: "check";
      readonly handle: number;
      readonly maxWork: number;
      readonly assumptions?: readonly Assumption[];
    }
  | {
      readonly kind: "assert";
      readonly handle: number;
      readonly expression: string;
    }
  | {
      readonly kind: "push" | "pop";
      readonly handle: number;
      readonly count: number;
    }
  | { readonly kind: "release"; readonly handle: number };
export type Reply =
  | { readonly kind: "script-results"; readonly results: readonly Solution[] }
  | { readonly kind: "solution"; readonly result: Solution }
  | { readonly kind: "prepared"; readonly handle: number }
  | { readonly kind: "released" }
  | { readonly kind: "updated" }
  | { readonly kind: "error"; readonly code: string; readonly message: string };
export const decode = (input: unknown): Reply => {
  if (!isRecord(input)) throw new Error("Invalid SMT reply");
  if (input.kind === "script-results") {
    if (!Array.isArray(input.results) || input.results.length > 256)
      throw new Error("Invalid SMT script results");
    return {
      kind: "script-results",
      results: input.results.map((result) =>
        solution(decode({ kind: "solution", result })),
      ),
    };
  }
  if (
    input.kind === "error" &&
    typeof input.code === "string" &&
    typeof input.message === "string"
  )
    return { kind: "error", code: input.code, message: input.message };
  if (input.kind === "released") return { kind: "released" };
  if (input.kind === "updated") return { kind: "updated" };
  if (
    input.kind === "prepared" &&
    typeof input.handle === "number" &&
    Number.isInteger(input.handle) &&
    input.handle > 0
  )
    return { kind: "prepared", handle: input.handle };
  const result = input.result;
  if (
    input.kind !== "solution" ||
    !isRecord(result) ||
    (result.status !== "sat" &&
      result.status !== "unsat" &&
      result.status !== "unknown") ||
    !isRecord(result.values) ||
    !isRecord(result.booleans) ||
    !isRecord(result.stats)
  )
    throw new Error("Invalid SMT solution");
  const values = Object.fromEntries(
    Object.entries(result.values).map(([name, value]) => {
      if (typeof value !== "string" || !/^(0|-?[1-9]\d*)$/.test(value))
        throw new Error("Invalid SMT integer");
      return [name, value];
    }),
  );
  const booleans = Object.fromEntries(
    Object.entries(result.booleans).map(([name, value]) => {
      if (typeof value !== "boolean") throw new Error("Invalid SMT Boolean");
      return [name, value];
    }),
  );
  const counts = [
    result.stats.work,
    result.stats.branches,
    result.stats.relaxations,
  ];
  if (
    counts.some(
      (value) =>
        typeof value !== "number" || !Number.isSafeInteger(value) || value < 0,
    )
  )
    throw new Error("Invalid SMT counts");
  if (
    result.status !== "sat" &&
    (Object.keys(values).length || Object.keys(booleans).length)
  )
    throw new Error("Unexpected SMT model");
  if (
    result.status === "unknown"
      ? result.reason !== "work"
      : result.reason !== undefined
  )
    throw new Error("Invalid SMT reason");
  return {
    kind: "solution",
    result: {
      status: result.status,
      values,
      booleans,
      ...(result.reason === undefined ? {} : { reason: String(result.reason) }),
      stats: {
        work: Number(result.stats.work),
        branches: Number(result.stats.branches),
        relaxations: Number(result.stats.relaxations),
      },
    },
  };
};
export const solution = (reply: Reply): Solution => {
  if (reply.kind !== "solution") throw new Error(JSON.stringify(reply));
  return reply.result;
};
