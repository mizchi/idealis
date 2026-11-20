import type {
  ExpansionStats,
  IncrementalSafetyResult,
  PortableCore,
  SafetyBounds,
  SafetyExpansionCache,
  SafetyProgram,
} from "./contracts.js";
import { normalizePortableValue, PortableCoreError } from "./portable-value.js";
import { checkSafety } from "./safety.js";
import { isRecord, valueKey } from "./trace.js";

const fields = (value: unknown, names: readonly string[]) => {
  if (
    !isRecord(value) ||
    Object.keys(value).length !== names.length ||
    names.some((name) => !Object.hasOwn(value, name))
  )
    throw new PortableCoreError("protocol", "Invalid expansion fields");
  return value;
};
const count = (value: unknown) => {
  if (
    typeof value !== "number" ||
    !Number.isInteger(value) ||
    value < 0 ||
    value > 0xffff_ffff
  )
    throw new PortableCoreError("protocol", "Invalid expansion count");
  return value;
};
export const createSafetyExpansionCache = (): SafetyExpansionCache => ({
  format: "idealis.safety-expansions",
  version: 1,
  buildDigest: "",
  transitionIdentity: "",
  entries: {},
});

export const decodeSafetyExpansionCache = (
  input: unknown,
): SafetyExpansionCache => {
  const value = fields(input, [
    "format",
    "version",
    "buildDigest",
    "transitionIdentity",
    "entries",
  ]);
  if (
    value.format !== "idealis.safety-expansions" ||
    value.version !== 1 ||
    typeof value.buildDigest !== "string" ||
    (value.buildDigest !== "" && !/^[0-9a-f]{64}$/.test(value.buildDigest)) ||
    typeof value.transitionIdentity !== "string" ||
    !isRecord(value.entries)
  )
    throw new PortableCoreError("protocol", "Invalid expansion cache");
  return {
    format: "idealis.safety-expansions",
    version: 1,
    buildDigest: value.buildDigest,
    transitionIdentity: value.transitionIdentity,
    entries: Object.fromEntries(
      Object.entries(value.entries).map(([key, raw]) => {
        const entry = fields(raw, ["digest", "successors"]);
        if (
          typeof entry.digest !== "string" ||
          !/^[0-9a-f]{64}$/.test(entry.digest) ||
          !Array.isArray(entry.successors)
        )
          throw new PortableCoreError("protocol", "Invalid complete expansion");
        return [
          key,
          {
            digest: entry.digest,
            successors: Array.from(entry.successors, (raw: unknown) => {
              const successor = fields(raw, ["action", "values"]);
              if (!Array.isArray(successor.values))
                throw new PortableCoreError(
                  "protocol",
                  "Invalid expansion state",
                );
              return {
                action: count(successor.action),
                values: Array.from(successor.values, (value: unknown) => {
                  const normalized = normalizePortableValue(value);
                  if (valueKey(value) !== valueKey(normalized))
                    throw new PortableCoreError(
                      "protocol",
                      "Noncanonical expansion value",
                    );
                  return normalized;
                }),
              };
            }),
          },
        ];
      }),
    ),
  };
};
export const decodeExpansionStats = (input: unknown): ExpansionStats => {
  const value = fields(input, ["hits", "misses", "writes", "invalidated"]);
  if (typeof value.invalidated !== "boolean")
    throw new PortableCoreError("protocol", "Invalid expansion invalidation");
  return {
    hits: count(value.hits),
    misses: count(value.misses),
    writes: count(value.writes),
    invalidated: value.invalidated,
  };
};

export const checkSafetyIncremental = (
  core: PortableCore,
  program: SafetyProgram,
  bounds: SafetyBounds,
  options: {
    readonly enabled?: boolean;
    readonly cache: SafetyExpansionCache;
    readonly buildDigest: string;
    readonly maxEntries: number;
  },
): IncrementalSafetyResult => {
  if (options.enabled !== true)
    return {
      result: checkSafety(core, program, bounds),
      cache: options.cache,
      stats: { hits: 0, misses: 0, writes: 0, invalidated: false },
    };
  const output = core.request({
    version: 1,
    request: {
      kind: "check-safety-incremental",
      program,
      bounds,
      enabled: true,
      cache: options.cache,
      buildDigest: options.buildDigest,
      maxEntries: options.maxEntries,
    },
  });
  if (output.kind !== "incremental-safety")
    throw new PortableCoreError(
      "protocol",
      "Expected incremental safety result",
    );
  return { result: output.result, cache: output.cache, stats: output.stats };
};

export const mergeSafetyExpansions = (
  core: PortableCore,
  left: SafetyExpansionCache,
  right: SafetyExpansionCache,
  maxEntries: number,
): SafetyExpansionCache => {
  const output = core.request({
    version: 1,
    request: { kind: "merge-safety-expansions", left, right, maxEntries },
  });
  if (output.kind !== "safety-expansions")
    throw new PortableCoreError("protocol", "Expected merged expansion cache");
  return output.cache;
};
