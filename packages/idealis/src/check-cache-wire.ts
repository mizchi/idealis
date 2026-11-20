import {
  parseBackendRequest,
  parseBackendResult,
  runBackend,
} from "./backend.js";
import { CheckCacheError } from "./check-cache-errors.js";
import type {
  BackendDescriptor,
  BackendRequest,
  BackendResult,
  CheckCacheArtifact,
  CheckCacheCell,
  CheckCacheJob,
} from "./contracts.js";
import { isRecord, valueKey } from "./trace.js";

export const cacheCount = (input: unknown, positive = false): number => {
  if (
    typeof input !== "number" ||
    !Number.isSafeInteger(input) ||
    input < (positive ? 1 : 0)
  )
    throw new CheckCacheError("Expected a safe cache counter");
  return input;
};
export const cacheText = (input: unknown): string => {
  if (typeof input !== "string" || !input.trim())
    throw new CheckCacheError("Expected a cache identifier");
  return input;
};
export const cacheFields = (input: unknown, names: readonly string[]) => {
  if (
    !isRecord(input) ||
    Object.keys(input).length !== names.length ||
    names.some((name) => !Object.hasOwn(input, name))
  )
    throw new CheckCacheError("Invalid cache fields");
  return input;
};
export const cacheDigest = async (input: unknown): Promise<string> =>
  Array.from(
    new Uint8Array(
      await globalThis.crypto.subtle.digest(
        "SHA-256",
        new TextEncoder().encode(valueKey(input)),
      ),
    ),
    (value) => value.toString(16).padStart(2, "0"),
  ).join("");

export const reusableResult = (result: BackendResult): boolean =>
  result.initialization === "nonempty" &&
  result.query.kind === "explicit" &&
  result.outcome.kind === "explicit-complete";

export const normalizeCacheJob = (input: CheckCacheJob): CheckCacheJob => {
  const raw = cacheFields(input, ["buildDigest", "backend", "request"]);
  if (
    typeof raw.buildDigest !== "string" ||
    !/^[0-9a-f]{64}$/.test(raw.buildDigest)
  )
    throw new CheckCacheError("Expected a backend build SHA-256");
  const backend = cacheFields(raw.backend, [
    "id",
    "version",
    "queries",
    "semantics",
  ]);
  if (
    !Array.isArray(backend.queries) ||
    !backend.queries.length ||
    backend.queries.some(
      (value) => !["explicit", "bounded", "induction"].includes(value),
    ) ||
    new Set(backend.queries).size !== backend.queries.length ||
    !Array.isArray(backend.semantics) ||
    !backend.semantics.length ||
    backend.semantics.some(
      (value) => typeof value !== "string" || !value.trim(),
    ) ||
    new Set(backend.semantics).size !== backend.semantics.length
  )
    throw new CheckCacheError("Invalid cached backend capabilities");
  const descriptor: BackendDescriptor = {
    id: cacheText(backend.id),
    version: cacheText(backend.version),
    queries: [...input.backend.queries],
    semantics: [...input.backend.semantics],
  };
  const request = parseBackendRequest(input.request);
  const snapshot = {
    buildDigest: raw.buildDigest,
    backend: descriptor,
    request,
  };
  valueKey(snapshot);
  const pending: unknown[] = [snapshot];
  const seen = new Set<object>();
  while (pending.length) {
    const value = pending.pop();
    if (Object.is(value, -0))
      throw new CheckCacheError("Signed zero is not a JSON cache input");
    if (typeof value !== "object" || value === null || seen.has(value))
      continue;
    seen.add(value);
    for (const child of Object.values(value)) pending.push(child);
    Object.freeze(value);
  }
  return snapshot;
};

export const identityJob = (identity: string): CheckCacheJob => {
  let raw: unknown;
  try {
    raw = JSON.parse(identity);
  } catch {
    throw new CheckCacheError("Invalid cache identity JSON");
  }
  const fields = cacheFields(raw, ["format", "version", "job"]);
  if (fields.format !== "idealis.check-cache-identity" || fields.version !== 1)
    throw new CheckCacheError("Unsupported cache identity");
  const job = cacheFields(fields.job, ["buildDigest", "backend", "request"]);
  const descriptor = cacheFields(job.backend, [
    "id",
    "version",
    "queries",
    "semantics",
  ]);
  if (!isRecord(job.request))
    throw new CheckCacheError("Invalid cached request");
  const normalized = normalizeCacheJob({
    buildDigest: cacheText(job.buildDigest),
    backend: descriptor as unknown as BackendDescriptor,
    request: job.request as unknown as BackendRequest,
  });
  if (
    valueKey({
      format: "idealis.check-cache-identity",
      version: 1,
      job: normalized,
    }) !== identity
  )
    throw new CheckCacheError("Noncanonical cache identity");
  return normalized;
};

export const validateCachedResult = (
  job: CheckCacheJob,
  result: unknown,
): Promise<BackendResult> =>
  runBackend(
    { descriptor: job.backend, check: async () => parseBackendResult(result) },
    job.request,
  );

export const artifactBody = (artifact: CheckCacheArtifact) => ({
  format: artifact.format,
  version: artifact.version,
  identity: artifact.identity,
  owner: artifact.owner,
  runId: artifact.runId,
  completedAt: artifact.completedAt,
  result: artifact.result,
});

export const decodeCacheCell = (input: unknown): CheckCacheCell => {
  const cell = cacheFields(input, [
    "version",
    "revision",
    "identity",
    "fence",
    "phase",
    "owner",
    "expiresAt",
    "artifact",
  ]);
  if (
    cell.version !== 1 ||
    (cell.phase !== "idle" &&
      cell.phase !== "leased" &&
      cell.phase !== "ready") ||
    typeof cell.owner !== "string"
  )
    throw new CheckCacheError("Invalid cache cell");
  const revision = cacheCount(cell.revision, true);
  const fence = cacheCount(cell.fence);
  const expiresAt = cacheCount(cell.expiresAt);
  const identity = cacheText(cell.identity);
  if (
    fence > revision ||
    (cell.phase !== "idle" && fence === 0) ||
    (cell.phase === "leased"
      ? !cell.owner || expiresAt === 0
      : cell.owner !== "" || expiresAt !== 0)
  )
    throw new CheckCacheError("Invalid cache lease state");
  const artifact =
    cell.artifact === null
      ? null
      : (() => {
          const entry = cacheFields(cell.artifact, [
            "format",
            "version",
            "identity",
            "owner",
            "runId",
            "completedAt",
            "result",
            "digest",
          ]);
          if (
            entry.format !== "idealis.check-cache-artifact" ||
            entry.version !== 1 ||
            typeof entry.digest !== "string" ||
            !/^[0-9a-f]{64}$/.test(entry.digest)
          )
            throw new CheckCacheError("Invalid cache artifact");
          const result = parseBackendResult(entry.result);
          if (!reusableResult(result) || entry.identity !== identity)
            throw new CheckCacheError(
              "Cache evidence is not a completed matching check",
            );
          return Object.freeze({
            format: "idealis.check-cache-artifact" as const,
            version: 1 as const,
            identity,
            owner: cacheText(entry.owner),
            runId: cacheText(entry.runId),
            completedAt: cacheCount(entry.completedAt),
            result,
            digest: entry.digest,
          });
        })();
  if ((cell.phase === "ready") !== (artifact !== null))
    throw new CheckCacheError("Invalid cache completion state");
  return Object.freeze({
    version: 1,
    revision,
    identity,
    fence,
    phase: cell.phase as CheckCacheCell["phase"],
    owner: cell.owner,
    expiresAt,
    artifact,
  });
};

export const validateCacheExchange = (
  previous: CheckCacheCell | null,
  expected: number | null,
  next: CheckCacheCell,
): void => {
  if (expected !== null) cacheCount(expected, true);
  if (
    next.revision !== cacheCount((expected ?? 0) + 1, true) ||
    (previous &&
      (previous.identity !== next.identity || previous.fence > next.fence))
  )
    throw new CheckCacheError(
      "Cache revisions and fences must advance without ABA",
    );
};
