import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { isAbsolute, join, relative, resolve } from "node:path";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { digest } from "../usable/provenance.js";

for (const directory of process.argv.slice(2)) {
  const result: unknown = JSON.parse(
    await readFile(join(directory, "result.json"), "utf8"),
  );
  const manifest: unknown = JSON.parse(
    await readFile(join(directory, "manifest.json"), "utf8"),
  );
  assert.ok(isRecord(result) && isRecord(manifest));
  assert.equal(result.status, "complete");
  assert.equal(result.exit, 0);
  assert.equal(result.sourceUnchanged, true);
  assert.equal(result.archiveError, null);
  assert.equal(result.sourceVersion, manifest.sourceVersion);
  assert.ok(Array.isArray(result.files) && Array.isArray(result.estimates));
  assert.equal(
    result.estimates.length,
    manifest.target === "rtla-compiler"
      ? manifest.benchmarkSuite === "finite-modules-operators-v1"
        ? 27
        : manifest.benchmarkSuite === "finite-and-modules-v1"
          ? 17
          : 12
      : manifest.target === "rtlc-coverage"
        ? 4
        : 14,
  );
  const files = new Set<string>();
  for (const file of result.files) {
    assert.ok(
      isRecord(file) &&
        typeof file.path === "string" &&
        typeof file.sha256 === "string",
    );
    const path = relative(resolve(directory), resolve(directory, file.path));
    assert.ok(
      path && !isAbsolute(path) && path !== ".." && !path.startsWith("../"),
    );
    assert.ok(!files.has(path));
    files.add(path);
    assert.equal(
      digest(await readFile(join(directory, path))),
      file.sha256,
      path,
    );
  }
  for (const path of ["manifest.json", "inputs.json", "criterion.log"])
    assert.ok(files.has(path));
  if (manifest.baselineRevision) assert.ok(files.has("source.tar"));
  if (
    manifest.benchmarkSuite === "finite-and-modules-v1" ||
    manifest.benchmarkSuite === "finite-modules-operators-v1"
  )
    assert.ok(files.has("module-inputs.json"));
  if (manifest.benchmarkSuite === "finite-modules-operators-v1")
    assert.ok(files.has("operator-inputs.json"));
  for (const estimate of result.estimates) {
    assert.ok(isRecord(estimate) && typeof estimate.benchmark === "string");
    const path = `criterion/${estimate.benchmark}/${String(result.stage)}/estimates.json`;
    assert.ok(files.has(path));
    const saved = JSON.parse(await readFile(join(directory, path), "utf8"));
    assert.deepEqual(
      [estimate.mean, estimate.lower, estimate.upper],
      [
        saved.mean.point_estimate,
        saved.mean.confidence_interval.lower_bound,
        saved.mean.confidence_interval.upper_bound,
      ],
    );
  }
  console.log(
    `${directory}: ${result.estimates.length} benchmarks, ${files.size} saved artifacts verified`,
  );
}
