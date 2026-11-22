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
export interface CacheStats {
  readonly hits: number;
  readonly misses: number;
  readonly writes: number;
  readonly invalidated: boolean;
}
/** Opaque check cache snapshot; the engine validates its contents. */
export type CheckCache = Readonly<Record<string, unknown>>;
export interface CacheFields {
  readonly cache?: CheckCache;
  readonly enabled?: boolean;
  readonly buildDigest?: string;
  readonly maxEntries?: number;
  /** An engine-side store from `cache-open`; excludes the fields above. */
  readonly cacheHandle?: number;
}
/** Where a reply leaves the cache: a snapshot, or the store it updated. */
export type CacheLocation =
  | { readonly cache: CheckCache; readonly cacheHandle?: never }
  | { readonly cacheHandle: number; readonly cache?: never };
export type Request =
  | ({
      readonly kind: "run-script-incremental";
      readonly source: string;
      readonly maxWork: number;
    } & CacheFields)
  | ({
      readonly kind: "check-incremental";
      readonly handle: number;
      readonly maxWork: number;
      readonly assumptions?: readonly Assumption[];
    } & CacheFields)
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
  | { readonly kind: "release"; readonly handle: number }
  | {
      readonly kind: "cache-open";
      readonly buildDigest: string;
      readonly maxEntries: number;
      readonly cache?: CheckCache;
    }
  | {
      readonly kind: "cache-export" | "cache-close";
      readonly cacheHandle: number;
    };
export type Reply =
  | ({
      readonly kind: "incremental-script-results";
      readonly results: readonly Solution[];
      readonly stats: CacheStats;
    } & CacheLocation)
  | ({
      readonly kind: "incremental-solution";
      readonly result: Solution;
      readonly stats: CacheStats;
    } & CacheLocation)
  | {
      readonly kind: "cache-opened";
      readonly cacheHandle: number;
      readonly entries: number;
      readonly invalidated: boolean;
    }
  | { readonly kind: "cache-snapshot"; readonly cache: CheckCache }
  | { readonly kind: "script-results"; readonly results: readonly Solution[] }
  | { readonly kind: "solution"; readonly result: Solution }
  | { readonly kind: "prepared"; readonly handle: number }
  | { readonly kind: "released" }
  | { readonly kind: "updated" }
  | { readonly kind: "error"; readonly code: string; readonly message: string };
const cacheStats = (input: unknown): CacheStats => {
  if (
    !isRecord(input) ||
    Object.keys(input).length !== 4 ||
    typeof input.invalidated !== "boolean"
  )
    throw new Error("Invalid SMT cache stats");
  const [hits, misses, writes] = [input.hits, input.misses, input.writes].map(
    (value) => {
      if (
        typeof value !== "number" ||
        !Number.isSafeInteger(value) ||
        value < 0
      )
        throw new Error("Invalid SMT cache counts");
      return value;
    },
  );
  return {
    hits: hits!,
    misses: misses!,
    writes: writes!,
    invalidated: input.invalidated,
  };
};
const checkCache = (input: unknown): CheckCache => {
  if (!isRecord(input)) throw new Error("Invalid SMT check cache");
  return input;
};
const count = (input: unknown, what: string): number => {
  if (typeof input !== "number" || !Number.isSafeInteger(input) || input < 0)
    throw new Error(`Invalid SMT ${what}`);
  return input;
};
/** Exactly one of `cache` and `cacheHandle`. */
const location = (input: Record<string, unknown>): CacheLocation => {
  if ("cacheHandle" in input === "cache" in input)
    throw new Error("Invalid SMT cache location");
  return "cacheHandle" in input
    ? { cacheHandle: count(input.cacheHandle, "cache handle") }
    : { cache: checkCache(input.cache) };
};
export const decode = (input: unknown): Reply => {
  if (!isRecord(input)) throw new Error("Invalid SMT reply");
  if (input.kind === "incremental-script-results") {
    const reply = decode({ kind: "script-results", results: input.results });
    if (reply.kind !== "script-results") throw new Error("Invalid SMT reply");
    return {
      kind: "incremental-script-results",
      results: reply.results,
      stats: cacheStats(input.stats),
      ...location(input),
    };
  }
  if (input.kind === "incremental-solution")
    return {
      kind: "incremental-solution",
      result: solution(decode({ kind: "solution", result: input.result })),
      stats: cacheStats(input.stats),
      ...location(input),
    };
  if (input.kind === "cache-opened") {
    if (typeof input.invalidated !== "boolean")
      throw new Error("Invalid SMT cache-opened reply");
    return {
      kind: "cache-opened",
      cacheHandle: count(input.cacheHandle, "cache handle"),
      entries: count(input.entries, "cache entries"),
      invalidated: input.invalidated,
    };
  }
  if (input.kind === "cache-snapshot")
    return { kind: "cache-snapshot", cache: checkCache(input.cache) };
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
