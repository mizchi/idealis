import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { gunzipSync } from "node:zlib";
import { nativePath, wasmPath } from "./clients.js";
import { digest, fingerprint } from "./provenance.js";
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
assert.deepEqual(
  manifest.seeds,
  Array.from(
    { length: manifest.phase === "development" ? 16 : 32 },
    (_, index) => (manifest.phase === "development" ? 0x36a1 : 0x36c3) + index,
  ),
);
const previous = JSON.parse(
  gunzipSync(await readFile(join(directory, "records.json.gz"))).toString(),
);
let index = 0;
const checked = await verify(manifest.seeds, (record) =>
  assert.deepEqual(record, previous[index++]),
);
assert.equal(index, previous.length);
assert.equal(checked.detected, result.faultDetected);
assert.deepEqual(
  checked.exchanges,
  JSON.parse(
    gunzipSync(await readFile(join(directory, "exchanges.json.gz"))).toString(),
  ),
);
console.log(
  JSON.stringify({
    status: "complete",
    seeds: checked.seeds,
    reproduced: checked.detected,
    exchanges: checked.exchanges.length,
  }),
);
