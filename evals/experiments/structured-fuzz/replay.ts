import assert from "node:assert/strict";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gunzipSync } from "node:zlib";
import {
  type File,
  type Record,
  replayAsan,
  summarize,
  validate,
} from "./campaign.js";
import { digest, fingerprint } from "./provenance.js";

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
for (const binary of manifest.binaries)
  assert.equal(digest(await readFile(binary.path)), binary.sha256, binary.path);
const corpus: File[] = JSON.parse(
  gunzipSync(await readFile(join(directory, "corpus.json.gz"))).toString(),
);
const expected: Record[] = JSON.parse(
  gunzipSync(await readFile(join(directory, "records.json.gz"))).toString(),
);
await replayAsan(
  await mkdtemp(join(tmpdir(), "idealis-fuzz-replay-")),
  corpus,
  manifest.seed,
);
let index = 0;
await validate(
  expected.map((record) => ({ source: record.source, input: record.input })),
  (record) => assert.deepEqual(record, expected[index++]),
);
assert.equal(index, expected.length);
assert.deepEqual(summarize(expected), result.summary);
console.log(
  JSON.stringify({
    status: "complete",
    corpus: corpus.length,
    nativeWasmRecords: index,
    asan: "all archived inputs replayed",
  }),
);
