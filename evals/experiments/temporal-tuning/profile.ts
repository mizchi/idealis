import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, loadavg } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { gunzipSync } from "node:zlib";
import { summarizeNative } from "../profile-tuning/hotspots.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { digest } from "../usable/provenance.js";

const binary = process.argv[2];
const directory = process.argv[3];
assert.ok(binary && directory);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const base = await fingerprint();
const sources = [
  ...base.sources,
  ...(await Promise.all(
    [
      "evals/plans/0049-temporal-tuning.md",
      "evals/experiments/temporal-tuning/profile.ts",
      "evals/experiments/profile-tuning/hotspots.ts",
    ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  )),
];
const sha256 = digest(await readFile(binary));
const files: string[] = [];
const save = async (path: string, value: unknown) => {
  await writeFile(join(directory, path), JSON.stringify(value, null, 2), {
    flag: "wx",
  });
  files.push(path);
};
await save("manifest.json", {
  plan: "0049-temporal-tuning",
  sourceVersion: digest(JSON.stringify(sources)),
  sources,
  commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
  binary,
  sha256,
  backend: "native",
  repetitions: 10000,
  samplingHz: 1000,
  cpu: cpus()[0]?.model,
  loadAverage: loadavg(),
  performanceMeasurement: false,
  build:
    "rtla-compiler Cargo release defaults, matching Criterion; symbols retained",
});
const inputs = JSON.parse(
  await readFile("crates/rtla-compiler/tests/operators-support.json", "utf8"),
);
const records = [];
try {
  for (const name of [
    "fair-forall",
    "fair-intermittent-WF",
    "fair-intermittent-SF",
  ]) {
    await save(
      `${name}-input.json`,
      inputs.find((input: { name: string }) => input.name === name),
    );
    const path = `${name}.json.gz`;
    const args: string[] = [
      "record",
      "--save-only",
      "--unstable-presymbolicate",
      "--rate",
      "1000",
      "--output",
      resolve(directory, path),
      resolve(binary),
      name,
      "10000",
    ];
    const child = spawnSync("samply", args, {
      env: { ...process.env, DYLD_LIBRARY_PATH: "/usr/lib" },
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    });
    await save(`${name}-process.json`, {
      args,
      environment: { DYLD_LIBRARY_PATH: "/usr/lib" },
      exit: child.status,
      stdout: child.stdout,
      stderr: child.stderr,
      error: child.error?.message ?? null,
    });
    assert.equal(child.error, undefined);
    assert.equal(child.status, 0, child.stderr);
    const observed = JSON.parse(child.stdout.trim());
    assert.equal(observed.repetitions, 10000);
    const profile = JSON.parse(
      gunzipSync(await readFile(join(directory, path))).toString(),
    );
    const symbolPath = path.replace(/\.gz$/, ".syms.json");
    const symbols = JSON.parse(
      await readFile(join(directory, symbolPath), "utf8"),
    );
    const samples = summarizeNative(profile, symbols, "samples");
    const cpu = summarizeNative(profile, symbols, "cpu");
    files.push(path, symbolPath);
    records.push({ input: name, path, observed, samples, cpu });
    console.log(`${name}: ${samples.totalWeight} samples`);
  }
  for (const source of sources)
    assert.equal(
      digest(await readFile(source.path)),
      source.sha256,
      source.path,
    );
  assert.equal(digest(await readFile(binary)), sha256);
  await save("result.json", {
    status: "complete",
    sourceVersion: digest(JSON.stringify(sources)),
    records,
    files: await Promise.all(
      files.map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "temporal-profile", status: "complete", binary: sha256 })}\n`,
  );
} catch (error) {
  await save("error.json", {
    status: "error",
    message: String(error),
    records,
  });
  throw error;
}
