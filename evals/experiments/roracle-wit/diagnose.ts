import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import type { SavedFixture } from "../roracle-native/measure.js";
import { snapshotModule } from "../snapshot-module.js";
import { decodeOracleReply } from "../workspace-oracle/protocol.js";
import type { encodeBatch, encodeBuffer } from "./encode.js";
import type { checkBuffer } from "./generated/native.js";
import type { instantiate } from "./generated/oracle.js";

const saved = process.argv[2];
const output = process.argv[3];
assert.ok(saved && output);
await mkdir(dirname(resolve(output)), { recursive: true });
await mkdir(output);
const packing: {
  readonly encodeBatch: typeof encodeBatch;
  readonly encodeBuffer: typeof encodeBuffer;
} = await import(snapshotModule(saved, "runtime", "roracle-wit/encode.js"));
const component: { readonly instantiate: typeof instantiate } = await import(
  pathToFileURL(resolve(saved, "generated/oracle.js")).href
);
const root = component.instantiate(
  (path) =>
    new WebAssembly.Module(readFileSync(resolve(saved, "generated", path))),
  {},
);
const native: { readonly checkBuffer: typeof checkBuffer } = createRequire(
  import.meta.url,
)(resolve(saved, "generated/roracle-typed.node"));
const fixtures: SavedFixture[] = JSON.parse(
  await readFile(resolve(saved, "fixtures.json"), "utf8"),
);
const records: {
  fixture: string;
  phase: string;
  sample: number;
  calls: number;
  totalNs: number;
}[] = [];
for (const fixture of fixtures.filter((fixture) =>
  ["single", "batch-32", "nested-256"].includes(fixture.name),
)) {
  const batch = packing.encodeBatch(fixture.input);
  const bytes = packing.encodeBuffer(batch);
  const callNative = () =>
    native.checkBuffer(
      Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength),
    );
  const raw = callNative();
  assert.deepEqual(decodeOracleReply(raw), fixture.expected);
  const phases = [
    {
      name: "pack",
      run: () => packing.encodeBuffer(packing.encodeBatch(fixture.input)),
      expected: bytes,
    },
    { name: "native-call", run: callNative, expected: raw },
    {
      name: "wit-record-call",
      run: () => root.protocol.check(batch),
      expected: raw,
    },
    {
      name: "wit-buffer-call",
      run: () => root.protocol.checkBuffer(bytes),
      expected: raw,
    },
  ];
  for (const phase of phases)
    for (let index = 0; index < 100; index++)
      assert.deepEqual(phase.run(), phase.expected);
  for (let sample = 0; sample < 20; sample++)
    for (const phase of [
      ...phases.slice(sample % 4),
      ...phases.slice(0, sample % 4),
    ]) {
      let totalNs = 0;
      for (let index = 0; index < fixture.iterations; index++) {
        const start = process.hrtime.bigint();
        const result = phase.run();
        totalNs += Number(process.hrtime.bigint() - start);
        assert.deepEqual(result, phase.expected);
      }
      records.push({
        fixture: fixture.name,
        phase: phase.name,
        sample,
        calls: fixture.iterations,
        totalNs,
      });
    }
}
await writeFile(
  resolve(output, "samples.json"),
  `${JSON.stringify(records, null, 2)}\n`,
  { flag: "wx" },
);
const rows = ["single", "batch-32", "nested-256"].map((fixture) => ({
  fixture,
  phases: Object.fromEntries(
    ["pack", "native-call", "wit-record-call", "wit-buffer-call"].map(
      (phase) => {
        const selected = records
          .filter((row) => row.fixture === fixture && row.phase === phase)
          .map((row) => row.totalNs / row.calls)
          .sort((left, right) => left - right);
        const first = selected[9];
        const second = selected[10];
        assert.ok(first && second);
        return [phase, (first + second) / 2000];
      },
    ),
  ),
}));
await writeFile(
  resolve(output, "summary.json"),
  `${JSON.stringify(rows, null, 2)}\n`,
  { flag: "wx" },
);
process.stdout.write(`${JSON.stringify(rows, null, 2)}\n`);
