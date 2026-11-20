import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { gunzipSync } from "node:zlib";
import { digest } from "../usable/provenance.js";
import { summarizeCpu, summarizeNative } from "./hotspots.js";

const saved = process.argv[2];
const directory = process.argv[3];
assert.ok(saved && directory);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const result = JSON.parse(await readFile(join(saved, "result.json"), "utf8"));
const manifest = JSON.parse(
  await readFile(join(saved, "manifest.json"), "utf8"),
);
assert.equal(result.status, "complete");
for (const file of result.files) {
  assert.ok(
    typeof file.path === "string" &&
      !file.path.includes("..") &&
      !file.path.startsWith("/"),
  );
  assert.equal(
    digest(await readFile(join(saved, file.path))),
    file.sha256,
    file.path,
  );
}
const summaries = [];
for (const record of result.records) {
  const profile = JSON.parse(
    gunzipSync(await readFile(join(saved, record.path))).toString(),
  );
  const symbols =
    manifest.backend === "native"
      ? JSON.parse(
          await readFile(
            join(saved, record.path.replace(/\.gz$/, ".syms.json")),
            "utf8",
          ),
        )
      : null;
  const samples =
    manifest.backend === "wasm"
      ? summarizeCpu(profile)
      : summarizeNative(profile, symbols, "samples");
  const cpu =
    manifest.backend === "native"
      ? summarizeNative(profile, symbols, "cpu")
      : null;
  summaries.push({
    input: record.input,
    workers: record.workers ?? 1,
    samples,
    cpu,
  });
}
await writeFile(
  join(directory, "result.json"),
  `${JSON.stringify({ status: "complete", saved, profileSourceVersion: result.sourceVersion, sourceResultSha256: digest(await readFile(join(saved, "result.json"))), selfUnit: "V8 sample count / native sample weight. Native cpu uses recorded thread CPU delta (microseconds). Inclusive counts recursion once per function.", summaries }, null, 2)}\n`,
  { flag: "wx" },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(directory), kind: "profile-summary", status: "complete", saved })}\n`,
);
