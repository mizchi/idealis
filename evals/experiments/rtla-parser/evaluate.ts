import assert from "node:assert/strict";
import {
  execFileSync,
  type SpawnSyncReturns,
  spawnSync,
} from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { digest } from "../usable/provenance.js";
import { compare } from "./compare.js";
import { archived } from "./corpus.js";
import { classifySany } from "./sany.js";
import { syntaxCases } from "./syntax.js";

const directory = process.argv[2];
const java = process.argv[3];
const jar = process.argv[4];
assert.ok(directory && java && jar);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const paths = execFileSync("git", [
  "ls-files",
  "-z",
  "--cached",
  "--others",
  "--exclude-standard",
  "--",
  "AGENTS.md",
  "crates/rtla",
  "evals/experiments/rtla-parser",
  "tests/rtla-parser",
  "evals/plans/0026-rtla-parser.md",
  "evals/experiments/collections-temporal/cases.ts",
  "evals/experiments/usable/provenance.ts",
  "packages/idealis/src/trace.ts",
  "tests/rtla-browser-entry.ts",
  "tests/rtla.browser.spec.ts",
  "playwright.config.ts",
  "scripts/package.ts",
  "scripts/package-test.ts",
  "scripts/release.ts",
  "justfile",
  "package.json",
  "packages/idealis/package.json",
  "pnpm-workspace.yaml",
  "pnpm-lock.yaml",
  "tsconfig.json",
  "packages/idealis/tsconfig.json",
])
  .toString()
  .split("\0")
  .filter(Boolean);
const sources = await Promise.all(
  [...new Set([...paths, ...archived])]
    .sort()
    .map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const sourceVersion = digest(JSON.stringify(sources));
const binaries = await Promise.all(
  [
    "crates/rtla/target/release/rtla",
    "crates/rtla/target/wasm32-unknown-unknown/release/rtla.wasm",
    jar,
  ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "0026-rtla-parser",
      sourceVersion,
      sources,
      binaries,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      dirty: execFileSync("git", ["status", "--porcelain"]).toString().trim(),
      environment: {
        node: process.version,
        platform: `${process.platform}/${process.arch}`,
        rustc: execFileSync("rustc", ["--version"]).toString().trim(),
      },
      reference: {
        tlaTs: "0.0.10",
        sanySource: "5a47802b5c391f59ecdd44117981f4ff8c0656ba",
        java: resolve(java),
      },
      normalization:
        "Compare semantic AST fields. Ignore source/range/spelling metadata, Number.text and rawConstants. Require modules/instances/uses to be empty. Keep operators, binding order, all values, prime, bullet nesting, assumptions and theorems.",
      startedAt: new Date().toISOString(),
    },
    null,
    2,
  ),
  { flag: "wx" },
);
try {
  const result = await compare();
  await writeFile(
    join(directory, "inputs.json"),
    JSON.stringify({ corpus: result.inputs, syntax: syntaxCases }),
    { flag: "wx" },
  );
  await writeFile(
    join(directory, "records.json"),
    JSON.stringify({
      corpus: result.records,
      syntax: result.syntaxRecords,
      limits: result.limitRecords,
    }),
    { flag: "wx" },
  );
  const sany = [];
  for (const input of syntaxCases) {
    const path = join(directory, "sany", input.name);
    await mkdir(path, { recursive: true });
    const file = join(path, "Syntax.tla");
    await writeFile(file, input.source, { flag: "wx" });
    const execution: SpawnSyncReturns<string> = spawnSync(
      resolve(java),
      ["-cp", resolve(jar), "tla2sany.SANY", "-s", resolve(file)],
      { encoding: "utf8" },
    );
    if (execution.error) throw execution.error;
    const output = execution.stdout + execution.stderr;
    await writeFile(join(path, "sany.log"), output, { flag: "wx" });
    const expected = input.expected !== "syntax";
    assert.equal(
      classifySany(output, execution.status),
      expected ? "accepted" : "syntax",
      `${input.name}: ${output}`,
    );
    sany.push({
      name: input.name,
      expected: expected ? "accepted" : "syntax-error",
      exitCode: execution.status,
      log: `sany/${input.name}/sany.log`,
    });
  }
  for (const entry of [...sources, ...binaries])
    assert.equal(digest(await readFile(entry.path)), entry.sha256, entry.path);
  const evidence = [
    "manifest.json",
    "inputs.json",
    "records.json",
    ...sany.flatMap((entry) => [
      entry.log,
      entry.log.replace(/sany.log$/, "Syntax.tla"),
    ]),
  ];
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      {
        status: "complete",
        sourceVersion,
        modules: result.moduleComparisons,
        configs: result.configComparisons,
        nativeWasmComparisons: result.nativeWasmComparisons,
        syntaxCases: result.syntaxRecords.length,
        limits: result.limitRecords.length,
        sany,
        files: await Promise.all(
          evidence.map(async (path) => ({
            path,
            sha256: digest(await readFile(join(directory, path))),
          })),
        ),
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "rtla-parser", status: "complete", sourceVersion })}\n`,
  );
  console.log(
    `${result.moduleComparisons} module/config pairs, ${result.nativeWasmComparisons} native/Wasm replies, ${sany.length} SANY syntax controls matched`,
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", sourceVersion, message: String(error) },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "rtla-parser", status: "error", sourceVersion })}\n`,
  );
  throw error;
}
