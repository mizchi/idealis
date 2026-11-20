import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { arch, platform } from "node:os";
import { dirname, resolve } from "node:path";
import type { SavedFixture } from "../roracle-native/measure.js";
import { snapshotModule } from "../snapshot-module.js";
import { type createEngine, modes } from "./engines.js";

interface Digest {
  readonly path: string;
  readonly sha256: string;
}
interface Manifest {
  readonly host: {
    readonly platform: string;
    readonly arch: string;
    readonly node: string;
  };
  readonly sources: readonly Digest[];
  readonly files: readonly Digest[];
}
interface Sample {
  readonly fixture: string;
  readonly mode: string;
  readonly sample: number;
  readonly calls: number;
  readonly totalNs: number;
}
interface Cold {
  readonly mode: string;
  readonly sample: number;
  readonly initAndFirstUs: number;
  readonly processMs: number;
}
const saved = process.argv[2];
const output = process.argv[3];
assert.ok(saved && output);
await mkdir(dirname(resolve(output)), { recursive: true });
await mkdir(output);
const manifest: Manifest = JSON.parse(
  await readFile(resolve(saved, "manifest.json"), "utf8"),
);
assert.equal(manifest.host.platform, platform());
assert.equal(manifest.host.arch, arch());
assert.equal(manifest.host.node, process.version);
for (const [root, files] of [
  [saved, manifest.files],
  [resolve(saved, "source"), manifest.sources],
] as const)
  for (const file of files) {
    assert.ok(
      !file.path.startsWith("/") && !file.path.split("/").includes(".."),
    );
    assert.equal(
      createHash("sha256")
        .update(await readFile(resolve(root, file.path)))
        .digest("hex"),
      file.sha256,
      file.path,
    );
  }
const module: { readonly createEngine: typeof createEngine } = await import(
  snapshotModule(saved, "runtime", "roracle-wit/engines.js")
);
const cases: SavedFixture[] = JSON.parse(
  await readFile(resolve(saved, "fixtures.json"), "utf8"),
);
assert.equal(cases.length, 12);
assert.equal(new Set(cases.map((fixture) => fixture.name)).size, 12);
for (const mode of modes) {
  const engine = await module.createEngine(mode, resolve(saved));
  for (const fixture of cases)
    assert.deepEqual(engine.request(fixture.input), fixture.expected);
}
const samples: Sample[] = JSON.parse(
  await readFile(resolve(saved, "samples.json"), "utf8"),
);
assert.equal(samples.length, 2160);
const median = (values: readonly number[]) => {
  assert.ok(
    values.length &&
      values.every((value) => Number.isFinite(value) && value > 0),
  );
  const sorted = [...values].sort((left, right) => left - right);
  const middle = sorted.length / 2;
  const first = sorted[Math.floor(middle)];
  const second = sorted[Math.ceil(middle) - 1];
  assert.ok(first && second);
  return (first + second) / 2;
};
const rows = cases.map((fixture) => ({
  fixture: fixture.name,
  modes: Object.fromEntries(
    modes.map((mode) => {
      const selected = samples.filter(
        (sample) => sample.fixture === fixture.name && sample.mode === mode,
      );
      assert.deepEqual(
        selected.map((sample) => sample.sample),
        Array.from({ length: 30 }, (_, index) => index),
      );
      assert.ok(
        selected.every(
          (sample) =>
            sample.calls === fixture.iterations &&
            sample.calls > 0 &&
            Number.isSafeInteger(sample.totalNs) &&
            sample.totalNs > 0,
        ),
      );
      return [
        mode,
        median(selected.map((sample) => sample.totalNs / sample.calls)) / 1000,
      ];
    }),
  ),
}));
const candidates = modes.slice(3).map((mode) => ({
  mode,
  versus: ["json-wasm", "native-json"].map((baseline) => {
    const ratios = rows.map((row) => {
      const before = row.modes[baseline];
      const after = row.modes[mode];
      assert.ok(before && after);
      return before / after;
    });
    const geometricMean = Math.exp(
      ratios.reduce((total, ratio) => total + Math.log(ratio), 0) /
        ratios.length,
    );
    return {
      baseline,
      geometricMean,
      worst: Math.min(...ratios),
      eligible: geometricMean >= 1.2 && Math.min(...ratios) >= 1 / 1.1,
    };
  }),
}));
const cold: Cold[] = JSON.parse(
  await readFile(resolve(saved, "cold.json"), "utf8"),
);
assert.equal(cold.length, 60);
const summary = {
  rows,
  candidates,
  cold: modes.map((mode) => {
    const selected = cold.filter((row) => row.mode === mode);
    assert.deepEqual(
      selected.map((row) => row.sample),
      Array.from({ length: 10 }, (_, index) => index),
    );
    return {
      mode,
      initAndFirstUs: median(selected.map((row) => row.initAndFirstUs)),
      processMs: median(selected.map((row) => row.processMs)),
    };
  }),
};
assert.deepEqual(
  summary,
  JSON.parse(await readFile(resolve(saved, "summary.json"), "utf8")),
);
await writeFile(
  resolve(output, "replay.json"),
  `${JSON.stringify({ status: "matched", reports: 72, samples: samples.length, summary }, null, 2)}\n`,
  { flag: "wx" },
);
process.stdout.write(
  "Replayed 72 complete reports, 2160 warm and 60 cold samples\n",
);
