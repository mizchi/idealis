import assert from "node:assert/strict";
import {
  execFileSync,
  type SpawnSyncReturns,
  spawnSync,
} from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, delimiter, join, resolve } from "node:path";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { referenceSource } from "../rtla-compatibility/reference.js";
import { createParserWasm } from "../rtla-parser/wasm.js";
import { digest } from "../usable/provenance.js";
import { cases, source } from "./corpus.js";

const [directory, javaHome, stableJar, previewJar, snapshot] =
  process.argv.slice(2);
assert.ok(directory && javaHome && stableJar && previewJar && snapshot);
await mkdir(directory);
await mkdir(join(directory, "reference"));
const save = (path: string, value: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const java = resolve(javaHome, "bin/java");
const adapter = resolve(directory, "reference/SyntaxOracle.java");
await writeFile(adapter, referenceSource, { flag: "wx" });
const compile = spawnSync(
  resolve(javaHome, "bin/javac"),
  ["-cp", resolve(stableJar), "-d", resolve(directory, "reference"), adapter],
  { encoding: "utf8" },
);
await save("reference/compile.json", compile);
assert.equal(compile.status, 0, compile.stderr);
const paths = execFileSync("git", [
  "ls-files",
  "--cached",
  "--others",
  "--exclude-standard",
  "--",
  "crates/rtla",
  "evals/experiments/symbolic-operators",
  "tests/symbolic-operators",
  "evals/experiments/rtla-compatibility/reference.ts",
  "evals/plans/0069-symbolic-operators.md",
])
  .toString()
  .trim()
  .split("\n")
  .sort();
const sources = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const binaries = await Promise.all(
  [
    join(snapshot, "rtla"),
    join(snapshot, "rtla.wasm"),
    stableJar,
    previewJar,
  ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
await save("manifest.json", {
  plan: "0069-symbolic-operators",
  sources,
  binaries,
  java: spawnSync(java, ["-version"], { encoding: "utf8" }).stderr,
  scope:
    "Fixed syntax controls only. Raw SANY trees are saved; no name/level/evaluation guarantee.",
  cases: cases.length,
});
await save("inputs.json", cases);
const wasm = await createParserWasm(
  await readFile(join(snapshot, "rtla.wasm")),
);
const records = [];
for (const input of cases) {
  await mkdir(join(directory, input.id));
  const path = join(input.id, "Symbolic.tla");
  await writeFile(join(directory, path), source(input.body), { flag: "wx" });
  const request = { kind: "module", source: source(input.body) };
  const native: SpawnSyncReturns<string> = spawnSync(
    resolve(snapshot, "rtla"),
    [],
    {
      encoding: "utf8",
      input: `${JSON.stringify(request)}\n`,
    },
  );
  await save(join(input.id, "native-process.json"), native);
  assert.equal(native.status, 0, native.stderr);
  const actual: unknown = JSON.parse(native.stdout);
  assert.deepEqual(wasm.request(request), actual, input.id);
  assert.ok(isRecord(actual));
  const kind =
    actual.kind === "module"
      ? "accepted"
      : isRecord(actual.error)
        ? actual.error.kind
        : undefined;
  assert.ok(kind === "accepted" || kind === "syntax" || kind === "unsupported");
  const references = [];
  for (const [version, jar] of [
    ["1.7.4", stableJar],
    ["1.8.0", previewJar],
  ]) {
    assert.ok(version && jar);
    const output = resolve(directory, input.id, `${version}.json`);
    const execution: SpawnSyncReturns<string> = spawnSync(
      java,
      [
        "-cp",
        `${resolve(directory, "reference")}${delimiter}${resolve(jar)}`,
        "tla2sany.parser.SyntaxOracle",
        resolve(directory, path),
        output,
      ],
      { encoding: "utf8", cwd: resolve(directory, "reference") },
    );
    await save(join(input.id, `${version}-process.json`), execution);
    assert.equal(execution.status, 0, execution.stderr);
    const response: unknown = JSON.parse(await readFile(output, "utf8"));
    assert.ok(
      isRecord(response) &&
        ["accepted", "syntax", "infrastructure"].includes(
          String(response.kind),
        ),
    );
    references.push({ version, kind: response.kind });
  }
  records.push({
    id: input.id,
    expected: input.accepted ? "accepted" : "syntax",
    rtla: kind,
    references,
    native: actual,
  });
}
for (const file of [...sources, ...binaries])
  assert.equal(digest(await readFile(file.path)), file.sha256, file.path);
const invalidControls = records.filter((record) =>
  record.references.some((reference) => reference.kind !== record.expected),
);
const mismatches = records.filter((record) =>
  record.references.some((reference) => reference.kind !== record.rtla),
);
const status = invalidControls.length
  ? "controls-failed"
  : mismatches.length
    ? "partial"
    : "complete";
await save("result.json", {
  status,
  compatibility: "partial",
  invalidControls,
  mismatches,
  records,
});
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(directory), kind: "symbolic-operator-controls", status, compatibility: "partial" })}\n`,
);
console.log({
  status,
  cases: records.length,
  invalidControls: invalidControls.map((record) => record.id),
  mismatches: mismatches.map((record) => record.id),
});
assert.equal(
  invalidControls.length,
  0,
  "Review independent reference controls",
);
if (process.env.IDEALIS_RTLA_SYMBOLS_REQUIRE_MATCH === "1")
  assert.equal(mismatches.length, 0);
