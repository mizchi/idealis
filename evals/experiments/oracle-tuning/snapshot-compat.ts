import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { snapshotModule } from "../snapshot-module.js";
import type { snapshot } from "../workspace-oracle/snapshot.js";
import type { createOracle } from "../workspace-oracle/wasm.js";
import { readOracleWasm } from "./wasm.js";

const before = process.argv[2];
const after = process.argv[3];
const output = process.argv[4];
assert.ok(before && after && output);
await mkdir(output);
const roots = [before, after].map((root) => resolve(root));
const engines = await Promise.all(
  roots.map(async (root) => {
    const module: { snapshot: typeof snapshot } = await import(
      snapshotModule(root, "dist", "workspace-oracle/snapshot.js")
    );
    const wasm: { createOracle: typeof createOracle } = await import(
      snapshotModule(root, "dist", "workspace-oracle/wasm.js")
    );
    const binary = await readOracleWasm(root);
    return {
      snapshot: module.snapshot,
      core: wasm.createOracle(binary),
      wasmSha256: createHash("sha256").update(binary).digest("hex"),
    };
  }),
);
const cycle: unknown[] = [];
cycle.push(cycle);
const nested = (depth: number): unknown =>
  depth === 0 ? null : [nested(depth - 1)];
const shared = { value: 7 };
const accepted: unknown[] = [
  undefined,
  null,
  true,
  false,
  0,
  -0,
  NaN,
  Infinity,
  -Infinity,
  1n,
  "雪😀",
  [],
  [undefined],
  {},
  { optional: undefined },
  JSON.parse('{"__proto__":7,"constructor":null}'),
  [shared, shared],
  Object.defineProperty([7], "0", { value: 7, enumerable: false }),
];
for (let index = 0; index < 100; index++)
  accepted.push({
    index,
    values: [
      index % 2 === 0,
      index / 3,
      String(index),
      { nested: [null, index] },
    ],
  });
const sparse: unknown[] = [];
sparse.length = 2;
const rejected = [
  cycle,
  nested(49),
  new Date(),
  new Map(),
  Symbol("x"),
  () => 1,
  sparse,
  Object.create(null),
  Object.defineProperty({}, "hidden", { value: 7 }),
  Object.defineProperty([7], "extra", { value: 7 }),
  Object.assign({}, { [Symbol("x")]: 7 }),
  Object.defineProperty({}, "field", {
    get: () => {
      throw new Error("getter invoked");
    },
    enumerable: true,
  }),
  Object.defineProperty([7], "0", {
    get: () => {
      throw new Error("getter invoked");
    },
    enumerable: true,
  }),
];
for (const input of rejected)
  for (const engine of engines) assert.throws(() => engine.snapshot(input));
for (const engine of engines)
  assert.doesNotThrow(() => engine.snapshot(nested(48)));
for (const text of ["\ud800", "\udc00"]) {
  assert.deepEqual(engines[0]?.snapshot(text), engines[1]?.snapshot(text));
  for (const engine of engines)
    assert.throws(() =>
      engine.core.request({
        version: 1,
        group: "unicode-refusal",
        reference: "reference",
        implementations: ["target"],
        cases: [
          {
            id: "one",
            input: null,
            reference: { kind: "value", value: engine.snapshot(text) },
            actual: [{ kind: "value", value: engine.snapshot(text) }],
          },
        ],
      }),
    );
}
const reports = [];
for (const [index, input] of accepted.entries()) {
  const snaps = engines.map((engine) => engine.snapshot(input));
  assert.deepEqual(snaps[0], snaps[1]);
  for (const mismatch of [false, true]) {
    const outputs = engines.map((engine) =>
      engine.core.request({
        version: 1,
        group: "snapshot-compat",
        reference: "reference",
        implementations: ["target"],
        cases: [
          {
            id: "one",
            input: { index, mismatch },
            reference: { kind: "value", value: engine.snapshot(input) },
            actual: [
              {
                kind: "value",
                value: engine.snapshot(mismatch ? { changed: input } : input),
              },
            ],
          },
        ],
      }),
    );
    assert.deepEqual(outputs[0], outputs[1]);
    assert.equal(outputs[0]?.status, mismatch ? "mismatch" : "matched");
    reports.push({ index, mismatch, report: outputs[0] });
  }
}
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify(
    {
      status: "matched",
      accepted: accepted.length,
      rejected: rejected.length,
      refusedSurrogates: 2,
      reportCount: reports.length,
      roots,
      wasmSha256: engines.map((engine) => engine.wasmSha256),
      reports,
    },
    null,
    2,
  )}\n`,
  { flag: "wx" },
);
console.log(
  `${reports.length} complete reports and ${rejected.length} refusals matched`,
);
