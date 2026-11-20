import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { wasmPath } from "../smt-subset/paths.js";
import { createWasm } from "../smt-subset/wasm.js";
import { digest, fingerprint } from "./provenance.js";
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
  ...result.files,
  ...manifest.binaries.map((binary: { path: string; sha256: string }) => ({
    ...binary,
    absolute: true,
  })),
])
  assert.equal(
    digest(
      await readFile(file.absolute ? file.path : join(directory, file.path)),
    ),
    file.sha256,
  );
const inputs = JSON.parse(
  await readFile(join(directory, "inputs.json"), "utf8"),
);
const saved = JSON.parse(
  await readFile(join(directory, "records.json"), "utf8"),
);
const records: unknown[] = [];
const summary = await verify(
  inputs,
  manifest.seed,
  await createWasm(await readFile(wasmPath)),
  (record) => records.push(record),
  manifest.binaries[2].path,
  await createWasm(await readFile(manifest.binaries[3].path)),
);
assert.deepEqual(summary, result.summary);
// Z3 may choose another valid model; all returned Rust models are revalidated.
const deterministic = (entries: Record<string, unknown>[]) =>
  entries.map((entry) =>
    entry.reference
      ? {
          ...entry,
          reference: { status: (entry.reference as { status: string }).status },
        }
      : entry,
  );
assert.deepEqual(
  deterministic(records as Record<string, unknown>[]),
  deterministic(saved),
);
console.log(JSON.stringify({ status: "replayed", ...summary }));
