import assert from "node:assert/strict";
import {
  appendFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { differential } from "../tlc-differential/differential.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";

const directory = process.argv[2];
const java = process.argv[3];
const jar = process.argv[4];
const saved = process.argv[5];
assert.ok(directory && java && jar);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const paths = [
  "evals/plans/0046-tlc-finite-support.md",
  ...(await readdir("evals/experiments/tlc-finite"))
    .filter((path) => path.endsWith(".ts"))
    .map((path) => `evals/experiments/tlc-finite/${path}`),
];
const base = await fingerprint();
const sources = [
  ...base.sources,
  ...(await Promise.all(
    paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  )),
].sort((left, right) => left.path.localeCompare(right.path));
const sourceVersion = digest(JSON.stringify(sources));
const binaries = await Promise.all(
  [
    java,
    jar,
    "crates/idealis-core/target/release/idealis-core",
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const inputs: {
  name: string;
  family: string;
  source: string;
  config: string;
  expected: string;
}[] = JSON.parse(
  await readFile("crates/rtla-compiler/tests/finite-support.json", "utf8"),
);
if (saved) {
  const previous = JSON.parse(
    await readFile(join(saved, "result.json"), "utf8"),
  );
  assert.equal(previous.status, "complete");
  assert.equal(previous.sourceVersion, sourceVersion);
  for (const file of previous.files)
    assert.equal(digest(await readFile(join(saved, file.path))), file.sha256);
  const manifest = JSON.parse(
    await readFile(join(saved, "manifest.json"), "utf8"),
  );
  assert.deepEqual(manifest.binaries, binaries);
  assert.deepEqual(manifest.inputs, inputs);
}
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "0046-tlc-finite-support",
      sourceVersion,
      sources,
      binaries,
      inputs,
      parent: saved ?? null,
      startedAt: new Date().toISOString(),
      performanceMeasurement: false,
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const core = await createPortableWasm(await readFile(binaries[3]?.path ?? ""));
const native = startNative();
const reports = [];
try {
  for (const input of inputs) {
    const target = join(directory, input.name);
    await mkdir(target);
    const report = await differential(
      { ...input, family: "graph" },
      core,
      native,
      java,
      jar,
      target,
      (source) => compileTlaSource(core, source),
    );
    reports.push(report);
    console.log(`${input.name}: ${report.differences.join(", ") || "matched"}`);
  }
  await writeFile(
    join(directory, "reports.json"),
    JSON.stringify(reports, null, 2),
    { flag: "wx" },
  );
  assert.ok(
    reports.every((report) => report.differences.length === 0),
    "TLC comparison failed",
  );
  for (const entry of sources)
    assert.equal(digest(await readFile(entry.path)), entry.sha256);
  if (saved)
    assert.deepEqual(
      reports,
      JSON.parse(await readFile(join(saved, "reports.json"), "utf8")),
    );
  const files: { path: string; sha256: string }[] = [];
  const collect = async (relative = ""): Promise<void> => {
    for (const entry of await readdir(join(directory, relative), {
      withFileTypes: true,
    })) {
      const path = join(relative, entry.name);
      if (entry.isDirectory()) await collect(path);
      else
        files.push({
          path,
          sha256: digest(await readFile(join(directory, path))),
        });
    }
  };
  await collect();
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      {
        status: "complete",
        sourceVersion,
        compared: reports.length,
        witnesses: reports.filter((report) => report.witness).length,
        files,
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "tlc-finite", status: "complete", sourceVersion })}\n`,
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify({ status: "error", error: String(error), reports }, null, 2),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "tlc-finite", status: "error", sourceVersion })}\n`,
  );
  throw error;
} finally {
  await native.close();
}
