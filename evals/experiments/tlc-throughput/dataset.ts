import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { verifyManifest } from "../../../packages/idealis/src/sharding.js";
import type { corpus } from "../parallel-safety/corpus.js";
import { digest } from "../usable/provenance.js";

export const loadDataset = async () => {
  const saved = "evals/runs/0017-parallel-safety";
  const result = JSON.parse(await readFile(join(saved, "result.json"), "utf8"));
  assert.equal(result.status, "complete");
  for (const path of ["inputs.json", "calibration.json"]) {
    const file = result.files.find(
      (file: { path: string }) => file.path === path,
    );
    assert.ok(file);
    assert.equal(digest(await readFile(join(saved, path))), file.sha256);
  }
  const data: Awaited<ReturnType<typeof corpus>> = JSON.parse(
    await readFile(join(saved, "inputs.json"), "utf8"),
  );
  await verifyManifest(data.manifest);
  const calibration = JSON.parse(
    await readFile(join(saved, "calibration.json"), "utf8"),
  );
  const mixed = new Set(
    data.manifest.tasks
      .filter((task) => task.payload.workload === "mixed")
      .map((task) => task.id),
  );
  const jobs = data.jobs
    .filter((job) => mixed.has(job.id))
    .map((job) => ({
      ...job,
      estimatedMs: Number(calibration.costs.native[job.program]),
    }));
  assert.equal(jobs.length, 88);
  assert.ok(
    jobs.every(
      (job) => Number.isFinite(job.estimatedMs) && job.estimatedMs > 0,
    ),
  );
  const ordered = [...jobs].sort(
    (left, right) => right.estimatedMs - left.estimatedMs,
  );
  return {
    parentRun: saved,
    parentSourceVersion: result.sourceVersion,
    inputs: data.inputs,
    parentManifest: data.manifest,
    jobs: ordered,
  };
};
