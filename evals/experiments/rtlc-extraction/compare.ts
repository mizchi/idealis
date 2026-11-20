import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  appendFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import type { PortableCommand } from "../../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyResult } from "../../../packages/idealis/src/safety.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";
import { assertOracle } from "../workspace-oracle/node.js";

interface Entry {
  readonly name: string;
  readonly command: PortableCommand;
}
const directory = process.argv[2];
assert.ok(directory);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const base = await fingerprint();
const additional = [
  "evals/plans/0025-rtlc-extraction.md",
  "evals/experiments/rtlc-extraction/compare.ts",
];
const sources = [
  ...base.sources,
  ...(await Promise.all(
    additional.map(async (path) => ({
      path,
      sha256: digest(await readFile(path)),
    })),
  )),
].sort((left, right) => left.path.localeCompare(right.path));
const sourceVersion = digest(JSON.stringify(sources));
const paths = [
  ".artifacts/rtlc-extraction/before/idealis-core",
  "crates/idealis-core/target/release/idealis-core",
  ".artifacts/rtlc-extraction/before/idealis_core.wasm",
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
];
const binaries = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const controls = ".artifacts/frontier-safety/controls";
const entries: Entry[] = await Promise.all(
  (await readdir(controls))
    .filter((path) => /^\d+-[1248]\.json$/.test(path))
    .sort((left, right) =>
      left.localeCompare(right, undefined, { numeric: true }),
    )
    .map(async (path) => {
      const entry: Entry = JSON.parse(
        await readFile(join(controls, path), "utf8"),
      );
      assert.equal(entry.command.request.kind, "check-safety-parallel");
      return entry;
    }),
);
assert.equal(entries.length, 241 * 4);
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "0025-rtlc-extraction",
      sourceVersion,
      sources,
      binaries,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      comparison:
        "Exact replies and work except parallel work cuts, whose explored prefix is scheduling-dependent. Preserve those replies and compare bound kind/cap.",
      startedAt: new Date().toISOString(),
    },
    null,
    2,
  ),
  { flag: "wx" },
);
await writeFile(join(directory, "inputs.json"), JSON.stringify(entries), {
  flag: "wx",
});
const before = startNative(paths[0]);
const after = startNative(paths[1]);
const wasmBefore = await createPortableWasm(await readFile(paths[2] ?? ""));
const wasmAfter = await createPortableWasm(await readFile(paths[3] ?? ""));
const capture = (
  core: typeof wasmBefore,
  command: PortableCommand,
): unknown => {
  try {
    return core.request(command);
  } catch (error) {
    assert.ok(
      error instanceof PortableCoreError,
      "Unexpected Wasm trap or host failure",
    );
    return { kind: "error", code: error.code, message: error.message };
  }
};
const records: unknown[] = [];
let nativeComparisons = 0;
let wasmComparisons = 0;
try {
  for (const entry of entries) {
    assert.ok(entry.command.request.kind === "check-safety-parallel");
    const expected = await before.request(entry.command);
    const actual = await after.request(entry.command);
    assert.ok(isRecord(expected) && isRecord(actual));
    if (expected.kind === "safety" && actual.kind === "safety") {
      const reference = decodeSafetyResult(expected.result);
      const result = decodeSafetyResult(actual.result);
      assert.ok(result.work <= entry.command.request.bounds.maxWork);
      if (
        entry.command.request.workers > 1 &&
        reference.status === "bounded" &&
        reference.reason === "work"
      ) {
        assert.equal(result.status, "bounded");
        assert.equal(result.reason, "work");
        assert.equal(result.violation, null);
      } else assertOracle(actual, expected, entry.name);
    } else assertOracle(actual, expected, entry.name);
    records.push({
      name: entry.name,
      backend: "native",
      workers: entry.command.request.workers,
      expected,
      actual,
    });
    nativeComparisons++;
    if (entry.command.request.workers === 1) {
      const command = {
        version: 1,
        request: {
          kind: "check-safety",
          program: entry.command.request.program,
          bounds: entry.command.request.bounds,
        },
      } as const;
      const expected = capture(wasmBefore, command);
      const actual = capture(wasmAfter, command);
      assertOracle(actual, expected, entry.name);
      records.push({ name: entry.name, backend: "wasm", expected, actual });
      wasmComparisons++;
    }
  }
  assert.equal(nativeComparisons, 964);
  assert.equal(wasmComparisons, 241);
  for (const source of sources)
    assert.equal(
      digest(await readFile(source.path)),
      source.sha256,
      source.path,
    );
  for (const binary of binaries)
    assert.equal(
      digest(await readFile(binary.path)),
      binary.sha256,
      binary.path,
    );
  await writeFile(join(directory, "records.json"), JSON.stringify(records), {
    flag: "wx",
  });
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      {
        status: "complete",
        sourceVersion,
        nativeComparisons,
        wasmComparisons,
        files: await Promise.all(
          ["manifest.json", "inputs.json", "records.json"].map(
            async (path) => ({
              path,
              sha256: digest(await readFile(join(directory, path))),
            }),
          ),
        ),
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "rtlc-extraction", status: "complete", sourceVersion })}\n`,
  );
  console.log(
    `${nativeComparisons} native + ${wasmComparisons} Wasm comparisons matched`,
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", message: String(error), records },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "rtlc-extraction", status: "error", sourceVersion })}\n`,
  );
  throw error;
} finally {
  await before.close();
  await after.close();
}
