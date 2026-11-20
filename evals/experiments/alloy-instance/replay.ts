import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fixed, generated } from "./corpus.js";
import { digest, fingerprint, nativePath, wasmPath } from "./provenance.js";
import { verify } from "./verify.js";

const directory = process.argv[2];
assert.ok(directory);
const result = JSON.parse(
  await readFile(join(directory, "result.json"), "utf8"),
);
assert.equal(result.status, "complete");
assert.equal(result.sourceVersion, (await fingerprint()).sourceVersion);
for (const file of result.files)
  assert.equal(
    digest(await readFile(join(directory, file.path))),
    file.sha256,
    file.path,
  );
const manifest = JSON.parse(
  await readFile(join(directory, "manifest.json"), "utf8"),
);
for (const path of [nativePath, wasmPath])
  assert.equal(
    digest(await readFile(path)),
    manifest.binaries.find((binary: { path: string }) => binary.path === path)
      ?.sha256,
  );
assert.ok(
  manifest.phase === "development" || manifest.phase === "confirmation",
);
const inputs = [
  ...fixed(),
  ...generated(
    manifest.phase === "development" ? 0x35a1 : 0x35c3,
    manifest.phase === "development" ? 16 : 32,
  ),
];
assert.deepEqual(inputs, manifest.inputs);
const records = JSON.parse(
  await readFile(join(directory, "records.json"), "utf8"),
);
assert.equal(records.length, inputs.length);
await verify(inputs, (input, result, index) => {
  assert.deepEqual(
    { input, result },
    { input: records[index]?.input, result: records[index]?.result },
  );
  assert.equal(records[index]?.reference.status, result.status);
});
console.log(
  JSON.stringify({
    status: "complete",
    cases: inputs.length,
    evidenceHashes: result.files.length,
    officialAnalyzer: "recorded receipts verified by digest, not rerun",
  }),
);
