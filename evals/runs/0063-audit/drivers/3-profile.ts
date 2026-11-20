import assert from "node:assert/strict";
import { type SpawnSyncReturns, spawnSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import { gunzipSync } from "node:zlib";
import { decodeSafetyProgram } from "../../src/safety-program.js";
import { isRecord } from "../../src/trace.js";
import { summarizeNative } from "../profile-tuning/hotspots.js";
import { fingerprint } from "../profile-tuning/provenance.js";
import { bounds } from "../single-model/cases.js";
import { digest } from "../usable/provenance.js";

const [binary, directory] = process.argv.slice(2);
assert.ok(binary && directory);
await mkdir(directory);
const planName = process.env.IDEALIS_CHECKER_PLAN ?? "0062-p0-checker";
const plan = await readFile(`evals/plans/${planName}.md`);
await writeFile(join(directory, "plan.md"), plan, { flag: "wx" });
const sources = await fingerprint();
const sha256 = digest(await readFile(binary));
const input = {
  source: await readFile("evals/fixtures/tcommit/TCommit.tla", "utf8"),
  config: await readFile("evals/fixtures/tcommit/TCommit.cfg", "utf8"),
};
const save = async (path: string, value: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  plan: planName,
  planSha256: digest(plan),
  sources,
  binary,
  sha256,
  sourceSha256: digest(input.source),
  configSha256: digest(input.config),
  driverSha256: digest(await readFile("evals/p0-checker/profile.ts")),
  repetitions: 16,
  samplingIntervalUs: 1000,
  scope:
    "Named native profile only. Includes IR decode/check/drop, not source compile or performance timing.",
});
const records = [];
for (const irVersion of [1, 2] as const) {
  const compiled: SpawnSyncReturns<string> = spawnSync(binary, [], {
    input: `${JSON.stringify({ version: 1, request: { kind: "compile-tla", ...input, irVersion } })}\n`,
    encoding: "utf8",
  });
  assert.equal(compiled.status, 0, compiled.stderr);
  const response: unknown = JSON.parse(compiled.stdout);
  assert.ok(isRecord(response) && response.kind === "compiled");
  const program = decodeSafetyProgram(response.program);
  const name = `tcommit-v${irVersion}`;
  const request = {
    version: 1,
    request: { kind: "check-safety", program, bounds },
  };
  const requests = `${JSON.stringify(request)}\n`.repeat(16);
  await writeFile(join(directory, `${name}-requests.jsonl`), requests, {
    flag: "wx",
  });
  const args: string[] = [
    "record",
    "--save-only",
    "--unstable-presymbolicate",
    "--rate",
    "1000",
    "--output",
    resolve(directory, `${name}.json.gz`),
    resolve(binary),
  ];
  const recorded: SpawnSyncReturns<string> = spawnSync("samply", args, {
    input: requests,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    env: { ...process.env, DYLD_LIBRARY_PATH: "/usr/lib" },
  });
  await save(`${name}-process.json`, {
    args,
    status: recorded.status,
    signal: recorded.signal,
    stdout: recorded.stdout,
    stderr: recorded.stderr,
    error: recorded.error?.message,
  });
  assert.equal(recorded.status, 0, recorded.stderr);
  const results: unknown[] = recorded.stdout
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  assert.equal(results.length, 16);
  for (const result of results) {
    assert.ok(
      isRecord(result) && result.kind === "safety" && isRecord(result.result),
    );
    assert.equal(result.result.status, "complete");
    assert.equal(result.result.states, 6816);
    assert.equal(result.result.generated, 53513);
  }
  const profile: Parameters<typeof summarizeNative>[0] = JSON.parse(
    gunzipSync(await readFile(join(directory, `${name}.json.gz`))).toString(),
  );
  const symbols: Parameters<typeof summarizeNative>[1] = JSON.parse(
    await readFile(join(directory, `${name}.json.syms.json`), "utf8"),
  );
  records.push({
    irVersion,
    results,
    samples: summarizeNative(profile, symbols, "samples"),
    cpu: summarizeNative(profile, symbols, "cpu"),
  });
}
assert.equal(digest(await readFile(binary)), sha256);
assert.deepEqual(await fingerprint(), sources);
await save("result.json", { status: "complete", records });
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(directory), plan: planName, kind: "tcommit-profile", status: "complete" })}\n`,
);
