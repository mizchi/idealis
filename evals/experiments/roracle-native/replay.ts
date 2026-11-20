import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { arch, platform } from "node:os";
import { dirname, resolve } from "node:path";
import {
  type ColdSample,
  createClient,
  modes,
  type Sample,
  type SavedFixture,
  summarize,
} from "./measure.js";

interface Digest {
  readonly path: string;
  readonly sha256: string;
}
interface Manifest {
  readonly version: number;
  readonly host: {
    readonly platform: string;
    readonly arch: string;
    readonly node: string;
  };
  readonly files: readonly Digest[];
  readonly sources: readonly Digest[];
}
const saved = process.argv[2];
const output = process.argv[3];
assert.ok(saved && output, "Provide saved and new output directories");
await mkdir(dirname(resolve(output)), { recursive: true });
await mkdir(output);
const manifest: Manifest = JSON.parse(
  await readFile(resolve(saved, "manifest.json"), "utf8"),
);
assert.equal(manifest.version, 1);
assert.equal(manifest.host.platform, platform());
assert.equal(manifest.host.arch, arch());
assert.equal(manifest.host.node, process.version);
for (const [base, files] of [
  [saved, manifest.files],
  [resolve(saved, "source"), manifest.sources],
] as const)
  for (const file of files) {
    assert.ok(
      !file.path.startsWith("/") && !file.path.split("/").includes(".."),
    );
    assert.equal(
      createHash("sha256")
        .update(await readFile(resolve(base, file.path)))
        .digest("hex"),
      file.sha256,
      `Archive digest: ${file.path}`,
    );
  }
const fixtures: SavedFixture[] = JSON.parse(
  await readFile(resolve(saved, "fixtures.json"), "utf8"),
);
assert.equal(fixtures.length, 6);
const wasm = await readFile(resolve(saved, "roracle.wasm"));
for (const mode of modes) {
  const client = createClient(
    mode,
    wasm,
    resolve(saved, "roracle-native.node"),
  );
  for (const fixture of fixtures)
    assert.deepEqual(client.request(fixture.input), fixture.expected);
}
const warm: Sample[] = JSON.parse(
  await readFile(resolve(saved, "warm.json"), "utf8"),
);
const cold: ColdSample[] = JSON.parse(
  await readFile(resolve(saved, "cold.json"), "utf8"),
);
const summary = summarize(fixtures, warm, cold);
assert.deepEqual(
  summary,
  JSON.parse(await readFile(resolve(saved, "summary.json"), "utf8")),
);
await writeFile(
  resolve(output, "replay.json"),
  `${JSON.stringify({ status: "matched", fixtures: fixtures.length, modes, samples: warm.length, coldSamples: cold.length, summary }, null, 2)}\n`,
  { flag: "wx" },
);
process.stdout.write(
  "Replayed 18 complete reports, source/binary digests and saved sample statistics\n",
);
