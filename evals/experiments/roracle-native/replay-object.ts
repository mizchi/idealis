import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { arch, platform } from "node:os";
import { dirname, resolve } from "node:path";
import { snapshotModule } from "../snapshot-module.js";
import { createOracle } from "../workspace-oracle/wasm.js";
import type { createNativeOracle } from "./client.js";
import type { SavedFixture } from "./measure.js";
import { type ObjectSample, summarizeObject } from "./object-summary.js";

interface Snapshot {
  readonly files: readonly { readonly path: string; readonly sha256: string }[];
}
interface Manifest {
  readonly before: Snapshot;
  readonly candidate: Snapshot;
  readonly wasmSha256: string;
  readonly host: {
    readonly platform: string;
    readonly arch: string;
    readonly node: string;
  };
  readonly control: boolean;
}
const before = process.argv[2];
const saved = process.argv[3];
const output = process.argv[4];
assert.ok(before && saved && output);
await mkdir(dirname(resolve(output)), { recursive: true });
await mkdir(output);
const manifest: Manifest = JSON.parse(
  await readFile(resolve(saved, "manifest.json"), "utf8"),
);
assert.equal(manifest.host.platform, platform());
assert.equal(manifest.host.arch, arch());
assert.equal(manifest.host.node, process.version);
assert.deepEqual(
  manifest.before,
  JSON.parse(await readFile(resolve(before, "manifest.json"), "utf8")),
);
for (const [root, snapshot] of [
  [before, manifest.before],
  [resolve(saved, "candidate"), manifest.candidate],
] as const)
  for (const file of snapshot.files) {
    assert.ok(
      !file.path.startsWith("/") && !file.path.split("/").includes(".."),
    );
    assert.equal(
      createHash("sha256")
        .update(await readFile(resolve(root, file.path)))
        .digest("hex"),
      file.sha256,
    );
  }
const bytes = await readFile(resolve(saved, "roracle.wasm"));
assert.equal(
  createHash("sha256").update(bytes).digest("hex"),
  manifest.wasmSha256,
);
const baseline: { readonly createNativeOracle: typeof createNativeOracle } =
  await import(snapshotModule(before, "dist", "roracle-native/client.js"));
const candidate: typeof baseline = await import(
  snapshotModule(saved, "candidate/dist", "roracle-native/client.js")
);
const native = resolve(saved, "candidate", "roracle-native.node");
const cases: SavedFixture[] = JSON.parse(
  await readFile(resolve(saved, "fixtures.json"), "utf8"),
);
assert.equal(cases.length, 12);
const targets = [
  createOracle(bytes),
  baseline.createNativeOracle("native-object", native),
  candidate.createNativeOracle("native-object", native),
  baseline.createNativeOracle("native-json", native),
  candidate.createNativeOracle("native-json", native),
];
for (const client of targets)
  for (const fixture of cases)
    assert.deepEqual(client.request(fixture.input), fixture.expected);
const samples: ObjectSample[] = JSON.parse(
  await readFile(resolve(saved, "samples.json"), "utf8"),
);
const summary = summarizeObject(cases, samples);
assert.deepEqual(
  summary,
  JSON.parse(await readFile(resolve(saved, "summary.json"), "utf8")),
);
await writeFile(
  resolve(output, "replay.json"),
  `${JSON.stringify({ status: "matched", reports: cases.length * targets.length, samples: samples.length, summary }, null, 2)}\n`,
  { flag: "wx" },
);
process.stdout.write(
  `Replayed ${cases.length * targets.length} full reports and ${samples.length} samples\n`,
);
