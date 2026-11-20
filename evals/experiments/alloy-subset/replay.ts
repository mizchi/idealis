import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { nativePath, wasmPath } from "./clients.js";
import { generated, handCases } from "./corpus.js";
import { digest, fingerprint } from "./provenance.js";
import { verify } from "./verify.js";
import { createWasm } from "./wasm.js";

const directory = process.argv[2];
assert.ok(directory);
const result = JSON.parse(
  await readFile(join(directory, "result.json"), "utf8"),
);
assert.equal(result.status, "complete");
assert.equal(result.sourceVersion, (await fingerprint()).sourceVersion);
for (const file of result.files) {
  assert.equal(
    digest(await readFile(join(directory, file.path))),
    file.sha256,
    file.path,
  );
}
const manifest = JSON.parse(
  await readFile(join(directory, "manifest.json"), "utf8"),
);
for (const path of [nativePath, wasmPath]) {
  assert.equal(
    digest(await readFile(path)),
    manifest.binaries.find((binary: { path: string }) => binary.path === path)
      ?.sha256,
  );
}
assert.ok(
  manifest.phase === "development" || manifest.phase === "confirmation",
);
const inputs = [
  ...handCases(),
  ...generated(
    manifest.phase === "development" ? 0x34a1 : 0x34c3,
    manifest.phase === "development" ? 32 : 64,
  ),
];
assert.deepEqual(inputs, manifest.inputs);
const records: unknown = JSON.parse(
  await readFile(join(directory, "records.json"), "utf8"),
);
assert.ok(Array.isArray(records));
let index = 0;
await verify(inputs, await createWasm(await readFile(wasmPath)), (record) => {
  const saved: unknown = records[index++];
  assert.ok(isRecord(saved));
  if ("input" in saved) {
    assert.deepEqual(record, { input: saved.input, result: saved.result });
    assert.ok(isRecord(saved.reference) && isRecord(saved.result));
    assert.equal(saved.reference.status, saved.result.status);
    if (saved.result.instance) assert.equal(saved.reference.pinned, true);
  } else assert.deepEqual(record, saved);
});
assert.equal(index, records.length);
console.log(
  JSON.stringify({
    status: "complete",
    replayed: index,
    evidenceHashes: result.files.length,
    officialAnalyzer: "recorded receipts verified by digest, not rerun",
  }),
);
