import { CheckCacheError } from "./check-cache-errors.js";
import { cacheCount, normalizeCacheJob } from "./check-cache-wire.js";
import type {
  CachedCheckExecution,
  CachedCheckSchedule,
  CachedCheckSchedulingJob,
  CheckCache,
  CheckCacheJob,
  SchedulingBudget,
  SchedulingOptions,
  SchedulingProfile,
  VerificationBackend,
} from "./contracts.js";
import { planSchedule } from "./scheduling.js";

export const planCachedChecks = async (
  cache: CheckCache,
  jobs: readonly CachedCheckSchedulingJob[],
  profiles: readonly SchedulingProfile[],
  budget: SchedulingBudget,
  options: SchedulingOptions,
  now: number,
): Promise<CachedCheckSchedule> => {
  if (new Set(jobs.map((job) => job.id)).size !== jobs.length)
    throw new CheckCacheError("Duplicate cached scheduling task");
  const started = performance.now();
  cacheCount(now);
  const profileSnapshot = profiles.map((profile) => ({ ...profile }));
  const budgetSnapshot = { ...budget };
  const optionSnapshot = { ...options };
  planSchedule(
    jobs.map((job) => ({
      id: job.id,
      estimatedMs: job.estimatedMs,
      memoryBytes: job.memoryBytes,
    })),
    profileSnapshot,
    budgetSnapshot,
    optionSnapshot,
  );
  const snapshots = jobs.map((job) => ({
    ...job,
    check: normalizeCacheJob(job.check),
  }));
  const probes = await Promise.all(
    snapshots.map((job) => cache.probe(job.check, now)),
  );
  const pending: CachedCheckSchedulingJob[] = [];
  const cached: CachedCheckSchedule["cached"][number][] = [];
  const deferred: CachedCheckSchedule["deferred"][number][] = [];
  for (const [index, job] of snapshots.entries()) {
    const probe = probes[index];
    if (!probe) throw new CheckCacheError("Missing cache probe");
    switch (probe.kind) {
      case "hit":
        cached.push({
          id: job.id,
          result: {
            backendResult: probe.artifact.result,
            origin: {
              kind: "cache",
              key: probe.key,
              producerRunId: probe.artifact.runId,
              artifactDigest: probe.artifact.digest,
            },
          },
        });
        break;
      case "busy":
        deferred.push({
          id: job.id,
          key: probe.key,
          expiresAt: probe.expiresAt,
        });
        break;
      case "miss":
        pending.push(job);
        break;
    }
  }
  const lookupMs = performance.now() - started;
  const plan = planSchedule(
    pending.map((job) => ({
      id: job.id,
      estimatedMs: job.estimatedMs,
      memoryBytes: job.memoryBytes,
    })),
    profileSnapshot,
    {
      ...budgetSnapshot,
      durationMs: Math.max(0, budgetSnapshot.durationMs - lookupMs),
    },
    optionSnapshot,
  );
  return { plan, pending, cached, deferred, lookupMs };
};

export const runCachedCheck = async <Input>(
  cache: CheckCache,
  job: CheckCacheJob,
  backend: VerificationBackend<Input>,
  options: {
    readonly owner: string;
    readonly runId: string;
    readonly leaseMs: number;
    readonly now: () => number;
  },
): Promise<CachedCheckExecution> => {
  const reservation = await cache.acquire(job, {
    owner: options.owner,
    runId: options.runId,
    now: options.now(),
    leaseMs: options.leaseMs,
  });
  switch (reservation.kind) {
    case "hit":
      return { kind: "cached", artifact: reservation.artifact };
    case "busy":
      return {
        kind: "deferred",
        key: reservation.key,
        expiresAt: reservation.expiresAt,
      };
    case "miss":
      throw new CheckCacheError("Acquire returned an unclaimed miss");
    case "lease": {
      let executed: Extract<CachedCheckExecution, { kind: "executed" }>;
      try {
        const receipt = await cache.execute(reservation.lease, backend);
        executed = {
          kind: "executed",
          result: receipt.result,
          publication: await cache.publish(
            reservation.lease,
            receipt,
            options.now(),
          ),
        };
      } catch (error) {
        try {
          await cache.release(reservation.lease);
        } catch (cleanupError) {
          throw new AggregateError(
            [error, cleanupError],
            "Check and lease cleanup failed",
            { cause: error },
          );
        }
        throw error;
      }
      if (executed.publication !== "stale") return executed;
      try {
        await cache.release(reservation.lease);
      } catch (cleanupError) {
        return { ...executed, cleanupError };
      }
      return executed;
    }
  }
};
