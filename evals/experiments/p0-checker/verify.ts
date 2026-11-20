import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { digest } from "../usable/provenance.js";

const directory = process.argv[2];
const planName = process.env.IDEALIS_CHECKER_PLAN ?? "0062-p0-checker";
const snapshotRoot =
  process.env.IDEALIS_CHECKER_SNAPSHOT ?? ".artifacts/0062-release";
assert.ok(directory);
await mkdir(directory);
const commands = [
  ["just", ["check"]],
  ["just", ["analysis-test"]],
  ["just", ["compiler-test"]],
  ["just", ["safety-test"]],
  ["just", ["quint-test"]],
  ["just", ["portable-test"]],
  ["just", ["browser"]],
  [
    "cargo",
    [
      "fmt",
      "--manifest-path",
      "crates/idealis-core/Cargo.toml",
      "--",
      "--check",
    ],
  ],
  [
    "cargo",
    [
      "clippy",
      "--all-targets",
      "--locked",
      "--manifest-path",
      "crates/idealis-core/Cargo.toml",
      "--",
      "-D",
      "warnings",
    ],
  ],
  ["just", ["runtime-build"]],
  ["just", ["runtime-test"]],
  ["just", ["mutation"]],
  ["just", ["evaluate"]],
  ["just", ["replay-evaluation"]],
  ["just", ["usable-test"]],
  ["just", ["usable-eval", join(directory, "usable")]],
  ["just", ["usable-replay", join(directory, "usable")]],
  ["just", ["cross-test"]],
  ["just", ["package"]],
  ["node", ["--test", "dist/scripts/check-support-test.js"]],
  ["just", ["support-check"]],
] as const;
const snapshots = JSON.parse(
  await readFile(join(snapshotRoot, "manifest.json"), "utf8"),
);
const before = await Promise.all(
  snapshots.sources.map(async (source: { path: string }) => ({
    path: source.path,
    sha256: digest(await readFile(source.path)),
  })),
);
const save = async (path: string, value: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  plan: planName,
  commands,
  sources: before,
  snapshots,
  driverSha256: digest(
    await readFile("evals/experiments/p0-checker/verify.ts"),
  ),
  sourceCheck: "Crate snapshots must not change during verification",
});
const results = [];
for (const [index, [command, args]] of commands.entries()) {
  const started = performance.now();
  const result = spawnSync(command, args, {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  const log = `${index}.log`;
  await writeFile(
    join(directory, log),
    `${result.stdout ?? ""}${result.stderr ?? ""}`,
    { flag: "wx" },
  );
  results.push({
    command,
    args,
    status: result.status,
    signal: result.signal,
    error: result.error?.message,
    wallMs: performance.now() - started,
    log,
    sha256: digest(await readFile(join(directory, log))),
  });
  console.log(
    `${index + 1}/${commands.length} ${command} ${args.join(" ")}: ${result.status}`,
  );
}
const after = await Promise.all(
  before.map(async (source: { path: string }) => ({
    path: source.path,
    sha256: digest(await readFile(source.path)),
  })),
);
const unchanged = JSON.stringify(before) === JSON.stringify(after);
const passed = unchanged && results.every((result) => result.status === 0);
await save("result.json", {
  status: passed ? "complete" : "failed",
  unchanged,
  results,
});
assert.ok(passed, "Inspect failed verification logs");
