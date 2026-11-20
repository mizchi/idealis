import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { gunzipSync } from "node:zlib";
import { summarizeNative } from "../profile-tuning/hotspots.js";
import { digest } from "../usable/provenance.js";

const [directory, snapshot] = process.argv.slice(2);
assert.ok(directory && snapshot);
await mkdir(directory);
const save = (name: string, value: unknown) =>
  writeFile(join(directory, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const binary = resolve(snapshot, "semantics-profile");
const sha256 = digest(await readFile(binary));
await save("manifest.json", {
  plan: "0072-frontend-semantics",
  snapshot,
  sha256,
  case: "action-repeat-6",
  repetitions: 100000,
  samplingCondition:
    "Increase repetitions after profiles 1 collected only 85/87 samples. Retain those original runs; this is diagnostic sampling, separate from timing.",
  driverSha256: digest(
    await readFile("evals/experiments/frontend-semantics/profile.ts"),
  ),
  scope:
    "Prepared search under sampling, separate from Criterion measurements.",
});
const args = [
  "record",
  "--save-only",
  "--unstable-presymbolicate",
  "--rate",
  "1000",
  "--output",
  resolve(directory, "profile.json.gz"),
  binary,
  "action-repeat-6",
  "100000",
];
const result = spawnSync("samply", args, {
  encoding: "utf8",
  maxBuffer: 16 * 1024 * 1024,
  env: { ...process.env, DYLD_LIBRARY_PATH: "/usr/lib" },
});
await save("process.json", {
  args,
  status: result.status,
  signal: result.signal,
  error: result.error?.message,
  stdout: result.stdout,
  stderr: result.stderr,
});
assert.equal(result.status, 0, result.stderr);
const profile: Parameters<typeof summarizeNative>[0] = JSON.parse(
  gunzipSync(await readFile(join(directory, "profile.json.gz"))).toString(),
);
const symbols: Parameters<typeof summarizeNative>[1] = JSON.parse(
  await readFile(join(directory, "profile.json.syms.json"), "utf8"),
);
assert.equal(digest(await readFile(binary)), sha256);
await save("result.json", {
  status: "complete",
  response: JSON.parse(result.stdout.trim()),
  samples: summarizeNative(profile, symbols, "samples"),
  cpu: summarizeNative(profile, symbols, "cpu"),
});
