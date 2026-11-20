import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import type { SavedFixture } from "../roracle-native/measure.js";
import { snapshotModule } from "../snapshot-module.js";
import type { createEngine, Mode } from "./engines.js";

const root = process.argv[2];
const mode: Mode = JSON.parse(process.argv[3] ?? "null");
assert.ok(root && mode);
const cases: SavedFixture[] = JSON.parse(
  await readFile(resolve(root, "fixtures.json"), "utf8"),
);
const input = cases[0];
assert.ok(input);
const start = process.hrtime.bigint();
const module: { readonly createEngine: typeof createEngine } = await import(
  snapshotModule(root, "runtime", "roracle-wit/engines.js")
);
const engine = await module.createEngine(mode, root);
const report = engine.request(input.input);
const initAndFirstUs = Number(process.hrtime.bigint() - start) / 1000;
assert.deepEqual(report, input.expected);
process.stdout.write(JSON.stringify({ report, initAndFirstUs }));
