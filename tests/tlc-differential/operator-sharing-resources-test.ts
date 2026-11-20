import assert from "node:assert/strict";
import { test } from "node:test";
import {
  assertAlive,
  mebibytes,
  runBounded,
} from "./operator-sharing-support.js";

test("resource evidence rejects an unmeasured zero-byte success", () => {
  assert.throws(
    () =>
      assertAlive({
        stdout: "{}",
        stderr: "",
        exit: 0,
        signal: null,
        elapsedMs: 0,
        peakRssBytes: 0,
        stopped: undefined,
      }),
    /maximum RSS/,
  );
});

test("resource evidence includes the child's OS high-water RSS", async () => {
  const run = await runBounded(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      `
    const allocation = Buffer.alloc(64 * 1024 * 1024, 1);
    process.stdout.write(JSON.stringify({
      maxRssBytes: process.resourceUsage().maxRSS * 1024,
      checksum: allocation[0] + allocation[allocation.length - 1],
    }));
  `,
    ],
    "",
    { milliseconds: 10000, bytes: mebibytes(256) },
  );
  assertAlive(run);
  assert.equal(run.exit, 0);
  const child = JSON.parse(run.stdout);
  assert.equal(child.checksum, 2);
  assert.ok(
    run.peakRssBytes >= child.maxRssBytes,
    `${run.peakRssBytes} < ${child.maxRssBytes}`,
  );
});
