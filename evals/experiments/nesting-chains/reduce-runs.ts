// Reduces the 0074 run records to the summaries that the iteration cites.
// Raw outputs (Criterion sample directories, per-case outputs, logs and large
// before/after dumps) are removed, and each run keeps `reduced.json` with the
// count, size and a digest of what was removed so that a copy of the raw
// records can be checked against it.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { join, relative, sep } from "node:path";

const [root = "evals/runs", source] = process.argv.slice(2);
assert.ok(source, "source revision that still holds the raw records");

const large = 256 * 1024;
const removedEverywhere = (path: string) =>
  path.endsWith(".log") || path.endsWith(".nohup");
const nested = (path: string) => path.includes("/");
const named =
  (...names: readonly string[]) =>
  (path: string) =>
    names.includes(path);

const rules: Readonly<Record<string, (path: string) => boolean>> = {
  // Criterion keeps a directory of samples per run of the ABBA order; the
  // tables, `criterion-summary.txt` and each bench's `runs.json` remain.
  "0074-abba-1": (path) => /^[^/]+\/\d-[^/]+\//.test(path),
  "0074-abba-2": (path) => /^[^/]+\/\d-[^/]+\//.test(path),
  "0074-abba-3": (path) => /^[^/]+\/\d-[^/]+\//.test(path),
  "0074-kernel": (path) => /^\d\//.test(path),
  // The tracked diffs are reproducible from Git; their digests remain.
  "0074-audit": named(
    "end-tracked.diff",
    "end2-tracked.diff",
    "start/tracked.diff",
  ),
  // Per-case responses; `result.json` and `0074-corpus-summary` remain.
  "0074-corpus": (path) =>
    !["manifest.json", "result.json", "plan.md", "ts-frontend.json"].includes(
      path,
    ),
  // Per-case TLC inputs and logs; `results.json`, `verdicts.json` and
  // `analysis.txt` remain. The inputs follow from fixtures/nesting-chains.
  "0074-tlc": nested,
  "0074-tlc-smoke": nested,
  "0074-ir-corpora": named("before.json", "after.json", "inputs.json"),
  "0074-r2-ir-corpora": named("after.json", "inputs.json"),
  "0074-paths": named("before.json", "after.json"),
  "0074-r2-paths": named("paths-after-r2.json"),
  "0074-temporal-parity": named("result.json"),
  "0074-browser": (path) => path.endsWith("/error-context.md"),
  // The generated TLA+ is identical to fixtures/nesting-chains/quint.
  "0074-quint-regen": (path) =>
    path.startsWith("apalache-out/") ||
    path.startsWith("server.") ||
    path.endsWith(".tla"),
};

const files = async (directory: string): Promise<string[]> => {
  const entries = await readdir(directory, {
    recursive: true,
    withFileTypes: true,
  });
  return entries
    .filter((entry) => entry.isFile())
    .map((entry) =>
      relative(directory, join(entry.parentPath, entry.name))
        .split(sep)
        .join("/"),
    )
    .sort((left, right) => (left < right ? -1 : left > right ? 1 : 0));
};
const sha256 = (value: Buffer | string) =>
  createHash("sha256").update(value).digest("hex");

const temporalSummary = async (directory: string) => {
  const result = JSON.parse(
    await readFile(join(directory, "result.json"), "utf8"),
  ) as {
    status: string;
    cases: number;
    commands: number;
    records: { name: string; responses: unknown[] }[];
  };
  return {
    status: result.status,
    cases: result.cases,
    commands: result.commands,
    records: result.records.map((record) => ({
      name: record.name,
      responses: record.responses.length,
      sha256: sha256(JSON.stringify(record.responses)),
    })),
  };
};

const runs = (await readdir(root, { withFileTypes: true }))
  .filter((entry) => entry.isDirectory() && entry.name.startsWith("0074-"))
  .map((entry) => entry.name)
  .sort();
let total = { files: 0, bytes: 0 };
for (const run of runs) {
  const directory = join(root, run);
  assert.ok(
    !existsSync(join(directory, "reduced.json")),
    `${run} is already reduced`,
  );
  const rule = rules[run];
  const removed: { path: string; bytes: number; sha256: string }[] = [];
  for (const path of await files(directory)) {
    if (!removedEverywhere(path) && !rule?.(path)) continue;
    const content = await readFile(join(directory, path));
    removed.push({ path, bytes: content.length, sha256: sha256(content) });
  }
  if (!removed.length) continue;
  if (run === "0074-temporal-parity")
    await writeFile(
      join(directory, "summary.json"),
      `${JSON.stringify(await temporalSummary(directory), null, 2)}\n`,
    );
  const bytes = removed.reduce((sum, file) => sum + file.bytes, 0);
  const record = {
    source,
    note: "Raw outputs removed from this run; the listing digest is sha256 over `sha256sum` lines of the removed files sorted by path (LC_ALL=C).",
    removed: {
      files: removed.length,
      bytes,
      listingSha256: sha256(
        removed.map((file) => `${file.sha256}  ${file.path}\n`).join(""),
      ),
      large: removed
        .filter((file) => file.bytes >= large)
        .map(({ path, bytes, sha256 }) => ({ path, bytes, sha256 })),
    },
  };
  await writeFile(
    join(directory, "reduced.json"),
    `${JSON.stringify(record, null, 2)}\n`,
    { flag: "wx" },
  );
  for (const file of removed) await rm(join(directory, file.path));
  for (const entry of (
    await readdir(directory, { recursive: true, withFileTypes: true })
  )
    .filter((entry) => entry.isDirectory())
    .map((entry) => join(entry.parentPath, entry.name))
    .sort((left, right) => right.length - left.length))
    if (!(await readdir(entry)).length) await rm(entry, { recursive: true });
  total = { files: total.files + removed.length, bytes: total.bytes + bytes };
  console.log(`${run}: removed ${removed.length} files, ${bytes} bytes`);
}
const kept = await Promise.all(
  runs.map(async (run) => {
    const paths = await files(join(root, run));
    const sizes = await Promise.all(
      paths.map(async (path) => (await stat(join(root, run, path))).size),
    );
    return { files: paths.length, bytes: sizes.reduce((a, b) => a + b, 0) };
  }),
);
console.log(
  JSON.stringify({
    removed: total,
    kept: kept.reduce(
      (sum, run) => ({
        files: sum.files + run.files,
        bytes: sum.bytes + run.bytes,
      }),
      { files: 0, bytes: 0 },
    ),
  }),
);
