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
import { createNative, maxWork, nativePath, wasmPath } from "./clients.js";
import { controls, generated, handCases, source } from "./corpus.js";
import { digest, fingerprint } from "./provenance.js";
import { reference } from "./reference.js";
import { createWasm } from "./wasm.js";
import { analysis } from "./wire.js";

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
const seed = phase === "development" ? 0x34a1 : 0x34c3;
const inputs = [
  ...handCases(),
  ...generated(seed, phase === "development" ? 32 : 64),
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
      binaries,
      phase,
      seed,
      parent,
      inputs,
      maxWork,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      environment: {
        node: process.version,
        platform: process.platform,
        arch: process.arch,
      },
      reference: "Alloy 6.2.0 / SAT4J",
      referenceSource: "59ba2033993449d483d54acad0e11a7bbf20354f",
      referenceDownload:
        "https://github.com/AlloyTools/org.alloytools.alloy/releases/download/v6.2.0/org.alloytools.alloy.dist.jar",
      referenceChecksum:
        "locally observed SHA256, release asset has no published digest",
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const native = createNative();
const wasm = await createWasm(await readFile(wasmPath));
const records: unknown[] = [];
let pinned = 0;
try {
  for (const [index, input] of inputs.entries()) {
    const request = {
      kind: "analyze" as const,
      source: source(input),
      command: 0,
      maxWork,
    };
    const result = analysis(await native.request(request));
    assert.deepEqual(wasm.request(request), { kind: "analysis", result });
    assert.equal(result.status, input.expected, input.name);
    assert.notEqual(result.status, "unknown");
    const checked = await reference(
      input,
      result,
      java,
      jar,
      join(directory, `case-${index}`),
    );
    if (checked.pinned) pinned++;
    records.push({ input, result, reference: checked });
    console.log(`${input.name}: ${result.status}`);
  }
  for (const source of controls) {
    const request = { kind: "analyze" as const, source, command: 0, maxWork };
    const result = await native.request(request);
    assert.equal(result.kind, "error", source);
    assert.deepEqual(wasm.request(request), result);
    records.push({ control: source, result });
  }
  const first = inputs[0];
  assert.ok(first);
  const unknown = analysis(
    wasm.request({
      kind: "analyze",
      source: source(first),
      command: 0,
      maxWork: 0,
    }),
  );
  assert.equal(unknown.status, "unknown");
  assert.equal(unknown.instance, null);
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
        path: file.slice(directory.length + 1),
        sha256: digest(await readFile(file)),
      })),
  );
  const result = {
    status: "complete",
    sourceVersion: stamp.sourceVersion,
    cases: inputs.length,
    controls: controls.length,
    pinned,
    budgetUnknown: true,
    files,
  };
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(result, null, 2),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    JSON.stringify({
      runId: basename(directory),
      kind: "ralloy-differential",
      status: "complete",
      sourceVersion: stamp.sourceVersion,
    }) + "\n",
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
    JSON.stringify({
      runId: basename(directory),
      kind: "ralloy-differential",
      status: "error",
      sourceVersion: stamp.sourceVersion,
    }) + "\n",
  );
  throw error;
} finally {
  await native.close();
}
