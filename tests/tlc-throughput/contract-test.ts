import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  normalizeTlc,
  tlcSettings,
} from "../../evals/experiments/tlc-throughput/protocol.js";

const reference = JSON.parse(
  await readFile(
    "evals/runs/0016-checker-performance/ring-1000000/process-2-tlc.json",
    "utf8",
  ),
);
test("TLC throughput requires success and counts from a real TLC log", () => {
  assert.deepEqual(normalizeTlc(reference.stdout, 0), {
    status: "complete",
    states: 1000000,
    generated: 1000001,
  });
  assert.throws(() => normalizeTlc(reference.stdout, 1));
  assert.throws(() =>
    normalizeTlc(
      reference.stdout.replace(
        /\d+ states generated, \d+ distinct states found/,
        "missing counts",
      ),
      0,
    ),
  );
  assert.throws(() =>
    normalizeTlc(
      reference.stdout.replace(
        "Model checking completed. No error has been found.",
        "Error: JVM parser failed",
      ),
      0,
    ),
  );
});
test("TLC configurations share eight exploration workers and eight GiB heap caps", () => {
  for (const route of ["tlc-1x8", "tlc-8x1"] as const) {
    const settings = tlcSettings(route);
    assert.equal(settings.jvmCount * settings.workers, 8);
    assert.equal(settings.jvmCount * settings.heapGiB, 8);
  }
});
