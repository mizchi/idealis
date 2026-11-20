import { runBackend } from "./backend.js";
import { CheckCacheError } from "./check-cache-errors.js";
import {
  advanceCachePolicy,
  type CachePolicyAction,
  type CachePolicyState,
} from "./check-cache-policy.js";
import {
  artifactBody,
  cacheCount,
  cacheDigest,
  cacheText,
  decodeCacheCell,
  identityJob,
  normalizeCacheJob,
  reusableResult,
  validateCachedResult,
  validateCacheExchange,
} from "./check-cache-wire.js";
import type {
  BackendRequest,
  CheckCache,
  CheckCacheArtifact,
  CheckCacheCell,
  CheckCacheJob,
  CheckCacheLease,
  CheckCacheProbe,
  CheckCacheStore,
  VerificationBackend,
} from "./contracts.js";
import { valueKey } from "./trace.js";

export { planCachedChecks, runCachedCheck } from "./cached-scheduling.js";
export { CheckCacheError } from "./check-cache-errors.js";
export type {
  CachedCheckExecution,
  CachedCheckSchedule,
  CachedCheckSchedulingJob,
  CheckCache,
  CheckCacheArtifact,
  CheckCacheCell,
  CheckCacheJob,
  CheckCacheLease,
  CheckCacheProbe,
  CheckCacheReceipt,
  CheckCacheReservation,
  CheckCacheStore,
} from "./contracts.js";

const policyState = (cell: CheckCacheCell): CachePolicyState => ({
  identity: cell.identity,
  fence: cell.fence,
  phase: cell.phase,
  owner: cell.owner,
  expiresAt: cell.expiresAt,
  storedIdentity: cell.artifact?.identity ?? "",
  complete: cell.artifact !== null && reusableResult(cell.artifact.result),
});
const action = (
  op: string,
  identity: string,
  values: Partial<CachePolicyAction> = {},
): CachePolicyAction => ({
  op,
  identity,
  owner: "",
  fence: 0,
  now: 0,
  duration: 1,
  receiptIdentity: "",
  complete: false,
  ...values,
});
const emptyCell = (identity: string): CheckCacheCell => ({
  version: 1,
  identity,
  revision: 0,
  fence: 0,
  phase: "idle",
  owner: "",
  expiresAt: 0,
  artifact: null,
});
const jobKey = async (job: CheckCacheJob) => {
  const identity = valueKey({
    format: "idealis.check-cache-identity",
    version: 1,
    job: normalizeCacheJob(job),
  });
  return { identity, key: await cacheDigest(identity) };
};
const checkLease = async (lease: CheckCacheLease) => {
  cacheCount(lease.fence, true);
  cacheText(lease.owner);
  cacheText(lease.runId);
  if ((await cacheDigest(lease.identity)) !== lease.key)
    throw new CheckCacheError("Lease identity does not match its key");
  return identityJob(lease.identity);
};

export const createMemoryCheckCacheStore = (): CheckCacheStore => {
  const cells = new Map<string, CheckCacheCell>();
  return {
    read: async (key) =>
      cells.has(key) ? structuredClone(cells.get(key) ?? null) : null,
    compareExchange: async (key, expected, input) => {
      const previous = cells.get(key) ?? null;
      if ((previous?.revision ?? null) !== expected) return false;
      const next = decodeCacheCell(input);
      validateCacheExchange(previous, expected, next);
      cells.set(key, next);
      return true;
    },
  };
};

export const createCheckCache = (store: CheckCacheStore): CheckCache => {
  const read = async (
    key: string,
    identity: string,
    verifyDigest = true,
  ): Promise<CheckCacheCell | null> => {
    const raw = await store.read(key);
    if (raw === null) return null;
    const cell = decodeCacheCell(raw);
    if (cell.identity !== identity)
      throw new CheckCacheError("Cache key collision or identity mismatch");
    if (
      verifyDigest &&
      cell.artifact &&
      (await cacheDigest(artifactBody(cell.artifact))) !== cell.artifact.digest
    )
      throw new CheckCacheError("Cache artifact digest mismatch");
    return cell;
  };
  const inspect = async (
    key: string,
    cell: CheckCacheCell | null,
    now: number,
  ): Promise<CheckCacheProbe> => {
    cacheCount(now);
    if (cell?.artifact) {
      const validated = await validateCachedResult(
        identityJob(cell.identity),
        cell.artifact.result,
      );
      if (
        !reusableResult(validated) ||
        valueKey(validated) !== valueKey(cell.artifact.result)
      )
        throw new CheckCacheError(
          "Cached claim is not supported by this backend request",
        );
      return { kind: "hit", key, artifact: cell.artifact };
    }
    return cell?.phase === "leased" && now < cell.expiresAt
      ? { kind: "busy", key, expiresAt: cell.expiresAt }
      : { kind: "miss", key };
  };
  const update = async (
    key: string,
    previous: CheckCacheCell | null,
    state: CachePolicyState,
    artifact: CheckCacheArtifact | null = null,
  ) =>
    store.compareExchange(key, previous?.revision ?? null, {
      version: 1,
      revision: cacheCount((previous?.revision ?? 0) + 1, true),
      identity: state.identity,
      fence: cacheCount(state.fence),
      phase: state.phase,
      owner: state.owner,
      expiresAt: cacheCount(state.expiresAt),
      artifact,
    });
  return {
    probe: async (job, now) => {
      const key = await jobKey(job);
      return inspect(key.key, await read(key.key, key.identity), now);
    },
    acquire: async (job, inputOptions) => {
      const options = { ...inputOptions };
      cacheCount(options.now);
      cacheCount(options.leaseMs, true);
      cacheCount(options.now + options.leaseMs, true);
      cacheText(options.owner);
      cacheText(options.runId);
      const key = await jobKey(job);
      for (;;) {
        const previous = await read(key.key, key.identity);
        const probe = await inspect(key.key, previous, options.now);
        if (probe.kind !== "miss") return probe;
        const next = advanceCachePolicy(
          policyState(previous ?? emptyCell(key.identity)),
          action("acquire", key.identity, {
            owner: options.owner,
            now: options.now,
            duration: options.leaseMs,
          }),
        );
        cacheCount(next.state.fence, true);
        if (await update(key.key, previous, next.state))
          return {
            kind: "lease",
            lease: Object.freeze({
              key: key.key,
              identity: key.identity,
              fence: next.state.fence,
              owner: options.owner,
              runId: options.runId,
            }),
          };
      }
    },
    execute: async <Input>(
      inputLease: CheckCacheLease,
      backend: VerificationBackend<Input>,
    ) => {
      const lease = Object.freeze({ ...inputLease });
      const job = await checkLease(lease);
      if (valueKey(backend.descriptor) !== valueKey(job.backend))
        throw new CheckCacheError(
          "Worker backend differs from the leased backend",
        );
      return Object.freeze({
        identity: lease.identity,
        fence: lease.fence,
        owner: lease.owner,
        runId: lease.runId,
        result: await runBackend(backend, job.request as BackendRequest<Input>),
      });
    },
    publish: async (inputLease, inputReceipt, now) => {
      const lease = Object.freeze({ ...inputLease });
      const receipt = structuredClone(inputReceipt);
      cacheCount(now);
      const job = await checkLease(lease);
      if (receipt.identity !== lease.identity)
        throw new CheckCacheError("Receipt belongs to a different request");
      if (
        receipt.fence !== lease.fence ||
        receipt.owner !== lease.owner ||
        receipt.runId !== lease.runId
      )
        throw new CheckCacheError("Receipt belongs to a different lease");
      const result = await validateCachedResult(job, receipt.result);
      const complete = reusableResult(result);
      const body = {
        format: "idealis.check-cache-artifact" as const,
        version: 1 as const,
        identity: lease.identity,
        owner: lease.owner,
        runId: lease.runId,
        completedAt: now,
        result,
      };
      const artifact = complete
        ? { ...body, digest: await cacheDigest(body) }
        : null;
      for (;;) {
        const previous = await read(lease.key, lease.identity);
        const next = advanceCachePolicy(
          policyState(previous ?? emptyCell(lease.identity)),
          action("publish", lease.identity, {
            owner: lease.owner,
            fence: lease.fence,
            now,
            receiptIdentity: receipt.identity,
            complete,
          }),
        );
        if (next.code === "stale") return "stale";
        if (next.code !== "published" && next.code !== "uncacheable")
          throw new CheckCacheError("Invalid cache publication");
        if (await update(lease.key, previous, next.state, artifact))
          return next.code;
      }
    },
    renew: async (inputLease, now, leaseMs) => {
      const lease = Object.freeze({ ...inputLease });
      await checkLease(lease);
      cacheCount(now);
      cacheCount(leaseMs, true);
      cacheCount(now + leaseMs, true);
      for (;;) {
        const previous = await read(lease.key, lease.identity);
        if (!previous) return false;
        const next = advanceCachePolicy(
          policyState(previous),
          action("renew", lease.identity, {
            owner: lease.owner,
            fence: lease.fence,
            now,
            duration: leaseMs,
          }),
        );
        if (next.code !== "renewed") return false;
        if (await update(lease.key, previous, next.state)) return true;
      }
    },
    release: async (inputLease) => {
      const lease = Object.freeze({ ...inputLease });
      await checkLease(lease);
      for (;;) {
        const previous = await read(lease.key, lease.identity);
        if (!previous) return false;
        const next = advanceCachePolicy(
          policyState(previous),
          action("release", lease.identity, {
            owner: lease.owner,
            fence: lease.fence,
          }),
        );
        if (next.code !== "released") return false;
        if (await update(lease.key, previous, next.state)) return true;
      }
    },
    evict: async (job) => {
      const key = await jobKey(job);
      for (;;) {
        const previous = await read(key.key, key.identity, false);
        if (!previous) return false;
        const next = advanceCachePolicy(
          policyState(previous),
          action("evict", key.identity),
        );
        if (next.code !== "evicted") return false;
        if (await update(key.key, previous, next.state)) return true;
      }
    },
  };
};
