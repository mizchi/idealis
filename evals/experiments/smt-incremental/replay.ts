import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { digest, fingerprint } from "./provenance.js";
import { compactRecords, expandRecords } from "./records.js";
import { verify } from "./verify.js";

const directory = process.argv[2];
assert.ok(directory);
const manifest = JSON.parse(
  await readFile(join(directory, "manifest.json"), "utf8"),
);
const result = JSON.parse(
  await readFile(join(directory, "result.json"), "utf8"),
);
assert.equal(result.status, "complete");
assert.equal(manifest.sourceVersion, (await fingerprint()).sourceVersion);
for (const file of [
  ...result.files.map((file: { path: string }) => ({
    ...file,
    path: join(directory, file.path),
  })),
  ...manifest.binaries,
])
  assert.equal(digest(await readFile(file.path)), file.sha256, file.path);
const inputs = JSON.parse(
  await readFile(join(directory, "inputs.json"), "utf8"),
);
const saved = (await readFile(join(directory, "records.jsonl"), "utf8"))
  .trim()
  .split("\n")
  .map((line) => JSON.parse(line));
const records: unknown[] = [];
const summary = await verify(inputs, (record) => records.push(record), {
  native: manifest.binaries[2].path,
  wasm: manifest.binaries[3].path,
});
assert.deepEqual(summary, result.summary);
// Z3 may choose another valid model; every Rust model is revalidated.
const deterministic = (entries: Record<string, unknown>[]) =>
  entries.map((entry) =>
    entry.reference
      ? {
          ...entry,
          reference: { status: (entry.reference as { status: string }).status },
        }
      : entry,
  );
assert.equal(manifest.recordFormat, "0074c-smt-compact-cache-records");
// Every saved cache delta must rebuild a snapshot with its recorded digest.
expandRecords(saved);
assert.deepEqual(
  deterministic(JSON.parse(JSON.stringify(compactRecords(records)))),
  deterministic(saved),
);
console.log(JSON.stringify({ status: "replayed", ...summary }));
