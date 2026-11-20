import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { digest } from "../usable/provenance.js";
import { readSnapshotSource } from "./snapshot-source.js";

const directory = process.argv[2];
const planName = process.env.IDEALIS_CHECKER_PLAN ?? "0072-frontend-semantics";
const snapshotRoot =
  process.env.IDEALIS_CHECKER_SNAPSHOT ?? ".artifacts/0072/final-snapshot";
assert.ok(directory);
await mkdir(directory);
const previous: {
  readonly unchanged: boolean;
  readonly results: readonly {
    readonly command: string;
    readonly args: readonly string[];
    readonly status: number;
  }[];
} = JSON.parse(
  await readFile("evals/runs/0072-verification/result.json", "utf8"),
);
assert.equal(previous.unchanged, true);
assert.deepEqual(
  previous.results
    .filter((result) => result.status !== 0)
    .map(({ command, args }) => [command, args]),
  [
    ["just", ["safety-test"]],
    ["node", ["--test", "dist/scripts/check-support-test.js"]],
    ["just", ["support-check"]],
  ],
);
const original: {
  readonly sources: readonly {
    readonly path: string;
    readonly sha256: string;
  }[];
} = JSON.parse(
  await readFile("evals/runs/0072-verification/manifest.json", "utf8"),
);
for (const source of original.sources.filter(
  (source) =>
    source.path.startsWith("src/") || source.path.startsWith("crates/"),
))
  assert.equal(
    digest(await readSnapshotSource(source.path)),
    source.sha256,
    source.path,
  );
const commands = [
  ["just", ["check"]],
  ["just", ["safety-test"]],
  ["just", ["mutation"]],
  ["node", ["--test", "dist/scripts/check-support-test.js"]],
  ["just", ["support-check"]],
  ["just", ["safety-eval", join(directory, "safety")]],
  ["just", ["safety-replay", join(directory, "safety")]],
  ["just", ["eval-loop"]],
  ["just", ["replay-loop"]],
] as const;
const snapshots = JSON.parse(
  await readFile(join(snapshotRoot, "manifest.json"), "utf8"),
);
const before = await Promise.all(
  snapshots.sources.map(async (source: { path: string }) => ({
    path: source.path,
    sha256: digest(await readSnapshotSource(source.path)),
  })),
);
assert.deepEqual(before, snapshots.sources);
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
    await readFile("evals/experiments/frontend-semantics/verify.ts"),
  ),
  sourceCheck:
    "Frozen non-Markdown inputs must not change. Original 18 passing gates retained; Rust crate and SDK sources are unchanged from that run.",
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
    sha256: digest(await readSnapshotSource(source.path)),
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
