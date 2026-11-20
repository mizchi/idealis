import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { gunzipSync } from "node:zlib";
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
assert.equal(manifest.sourceVersion, result.sourceVersion);
assert.ok(
  manifest.phase === "development" || manifest.phase === "confirmation",
);
assert.deepEqual(
  manifest.seeds,
  Array.from(
    { length: manifest.phase === "development" ? 16 : 32 },
    (_, index) => (manifest.phase === "development" ? 0x38a1 : 0x38c3) + index,
  ),
);
for (const binary of manifest.binaries)
  assert.equal(digest(await readFile(binary.path)), binary.sha256, binary.path);
const records = JSON.parse(
  gunzipSync(await readFile(join(directory, "records.json.gz"))).toString(),
);
let index = 0;
const checked = await verify(manifest.seeds, (record) =>
  assert.deepEqual(record, records[index++]),
);
assert.equal(index, records.length);
assert.deepEqual(
  checked.exchanges,
  JSON.parse(
    gunzipSync(await readFile(join(directory, "exchanges.json.gz"))).toString(),
  ),
);
assert.deepEqual(
  checked.sutExchanges,
  JSON.parse(
    gunzipSync(
      await readFile(join(directory, "sut-exchanges.json.gz")),
    ).toString(),
  ),
);
const loomPath =
  "evals/experiments/controlled-schedules/rust/target/release/examples/loom";
assert.deepEqual(
  JSON.parse(execFileSync(loomPath, { encoding: "utf8" })),
  JSON.parse(await readFile(join(directory, "loom.json"), "utf8")),
);
for (const key of [
  "seeds",
  "normal",
  "detected",
  "missed",
  "exhaustive",
] as const)
  assert.deepEqual(checked[key], result[key], key);
console.log(
  JSON.stringify({
    status: "complete",
    normal: checked.normal,
    detected: checked.detected,
    missed: checked.missed,
    records: index,
    exchanges: checked.exchanges.length,
    sutExchanges: checked.sutExchanges.length,
  }),
);
