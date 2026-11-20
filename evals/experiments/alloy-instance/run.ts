import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  appendFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, join, relative } from "node:path";
import { fixed, generated } from "./corpus.js";
import { digest, fingerprint, nativePath, wasmPath } from "./provenance.js";
import { reference } from "./reference.js";
import { verify } from "./verify.js";

const phase = process.argv[2];
const directory = process.argv[3];
const java = process.argv[4];
const jar = process.argv[5];
const parent = process.argv[6];
assert.ok(
  (phase === "development" || phase === "confirmation") &&
    directory &&
    java &&
    jar,
);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const stamp = await fingerprint();
if (phase === "confirmation") {
  assert.ok(parent);
  const previous = JSON.parse(
    await readFile(join(parent, "result.json"), "utf8"),
  );
  assert.equal(previous.status, "complete");
  assert.equal(previous.sourceVersion, stamp.sourceVersion);
}
const seed = phase === "development" ? 0x35a1 : 0x35c3;
const inputs = [
  ...fixed(),
  ...generated(seed, phase === "development" ? 16 : 32),
];
const binaries = await Promise.all(
  [nativePath, wasmPath, java, jar].map(async (path) => ({
    path,
    sha256: digest(await readFile(path)),
    bytes: (await readFile(path)).length,
  })),
);
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      ...stamp,
      phase,
      seed,
      inputs,
      binaries,
      parent,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      environment: {
        node: process.version,
        platform: process.platform,
        arch: process.arch,
      },
      reference: {
        version: "Alloy 6.2.0 / SAT4J",
        source: "59ba2033993449d483d54acad0e11a7bbf20354f",
        jarChecksum: "locally observed SHA256, release has no official digest",
      },
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const records: unknown[] = [];
try {
  await verify(inputs, async (input, result, index) => {
    const checked = await reference(
      input,
      result,
      java,
      jar,
      join(directory, `case-${index}`),
    );
    records.push({ input, result, reference: checked });
    console.log(`${input.name}: ${result.status}/${result.reason}`);
  });
  assert.equal((await fingerprint()).sourceVersion, stamp.sourceVersion);
  await writeFile(
    join(directory, "records.json"),
    JSON.stringify(records, null, 2),
    { flag: "wx" },
  );
  const files = await Promise.all(
    (await readdir(directory, { recursive: true, withFileTypes: true }))
      .filter((entry) => entry.isFile())
      .map((entry) => join(entry.parentPath, entry.name))
      .sort()
      .map(async (file) => ({
        path: relative(directory, file),
        sha256: digest(await readFile(file)),
      })),
  );
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      {
        status: "complete",
        sourceVersion: stamp.sourceVersion,
        cases: inputs.length,
        budgetUnknown: true,
        files,
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "alloy-instance", status: "complete", sourceVersion: stamp.sourceVersion })}\n`,
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
    `${JSON.stringify({ runId: basename(directory), kind: "alloy-instance", status: "error", sourceVersion: stamp.sourceVersion })}\n`,
  );
  throw error;
}
