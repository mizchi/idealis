import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { isDeepStrictEqual } from "node:util";
import { runInNewContext } from "node:vm";
import {
  assertOracle,
  compareOracle,
} from "../../evals/experiments/workspace-oracle/node.js";
import { snapshot } from "../../evals/experiments/workspace-oracle/snapshot.js";
import { createOracle } from "../../evals/experiments/workspace-oracle/wasm.js";

test("oracle equality retains the previous strict equality on portable snapshots", () => {
  const values = [
    undefined,
    null,
    true,
    false,
    0,
    -0,
    1,
    NaN,
    Infinity,
    -Infinity,
    1n,
    "1",
    [],
    [undefined],
    { optional: undefined },
    {},
    { b: 2, a: [1, "x"] },
    { a: [1, "x"], b: 2 },
  ];
  for (const actual of values)
    for (const expected of values) {
      assert.equal(
        compareOracle(actual, expected).status === "matched",
        isDeepStrictEqual(actual, expected),
      );
    }
  assert.doesNotThrow(() => assertOracle({ a: 1, b: 2 }, { b: 2, a: 1 }));
  assert.throws(
    () => assertOracle({ value: 2 }, { value: 1 }, "count drift"),
    (error) => {
      assert.ok(error instanceof assert.AssertionError);
      assert.match(error.message, /count drift/);
      assert.ok("report" in error);
      return true;
    },
  );
});

test("nonportable values and cycles fail explicitly instead of losing data", () => {
  const cycle: unknown[] = [];
  cycle.push(cycle);
  const sparse: unknown[] = [];
  sparse.length = 2;
  for (const value of [
    cycle,
    new Date(),
    new Map(),
    () => 1,
    Symbol("x"),
    sparse,
  ])
    assert.throws(() => snapshot(value));
  assert.throws(
    () =>
      snapshot(
        Object.defineProperty({}, "value", {
          get: () => {
            throw new Error("getter must not run");
          },
          enumerable: true,
        }),
      ),
    /accessor/,
  );
});

test("batch wire preserves unknown and execution failure without matching them", async () => {
  const core = createOracle(
    await readFile(
      "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
    ),
  );
  for (const kind of ["unknown", "invalid", "error"]) {
    const observation =
      kind === "error" ? { kind, message: "fault" } : { kind, reason: "fault" };
    const report = core.request({
      version: 1,
      group: "negative",
      reference: "reference",
      implementations: ["candidate"],
      cases: [
        {
          id: "one",
          input: null,
          reference: observation,
          actual: [observation],
        },
      ],
    });
    assert.equal(report.status, kind);
  }
  assert.throws(() =>
    core.request({
      version: 1,
      group: "empty",
      reference: "reference",
      implementations: [],
      cases: [],
    }),
  );
});

test("declared workspace migration sites all use the shared oracle", async () => {
  const inventory: readonly { path: string; equalitySites: number }[] =
    JSON.parse(
      await readFile("fixtures/workspace-oracle/migrations.json", "utf8"),
    );
  for (const item of inventory) {
    const source = await readFile(item.path, "utf8");
    assert.equal(
      [...source.matchAll(/\b(?:assertOracle|compareOracle)\(/g)].length,
      item.equalitySites,
      item.path,
    );
  }
});

test("mutating an input is observed on the next call and does not alter a previous report", () => {
  const actual = { nested: [1, 2] };
  const expected = { nested: [1, 2] };
  const previous = compareOracle(actual, expected);
  const saved = JSON.stringify(previous);
  actual.nested.push(3);
  assert.equal(previous.status, "matched");
  assert.equal(compareOracle(actual, expected).status, "mismatch");
  assert.equal(JSON.stringify(previous), saved);
  expected.nested.push(3);
  assert.equal(compareOracle(actual, expected).status, "matched");
});

test("snapshot traversal preserves special keys, shared values and exact depth", () => {
  const special = JSON.parse(
    '{"__proto__":{"polluted":true},"constructor":null,"日本語":"雪"}',
  );
  const saved = snapshot(special);
  assert.deepEqual(saved, {
    type: "object",
    value: {
      ["__proto__"]: {
        type: "object",
        value: { polluted: { type: "boolean", value: true } },
      },
      constructor: { type: "null" },
      日本語: { type: "string", value: "雪" },
    },
  });
  assert.equal(Object.getPrototypeOf(saved.value), Object.prototype);
  const shared = { item: 7 };
  assert.deepEqual(
    snapshot([shared, shared]),
    snapshot([{ item: 7 }, { item: 7 }]),
  );
  const hiddenIndex = Object.defineProperty([7], "0", {
    value: 7,
    enumerable: false,
  });
  assert.deepEqual(snapshot(hiddenIndex), snapshot([7]));
  const nested = (depth: number): unknown =>
    depth === 0 ? null : [nested(depth - 1)];
  assert.doesNotThrow(() => snapshot(nested(48)));
  assert.throws(() => snapshot(nested(49)), /nesting exceeds 48/);
});

test("snapshot rejects hidden fields, extended arrays and accessors without invoking them", () => {
  const trailingHole = [7];
  trailingHole.length = 2;
  for (const value of [
    Object.defineProperty({}, "hidden", { value: 7 }),
    Object.defineProperty([7], "hidden", { value: 7 }),
    Object.defineProperty([7], "extra", { value: 7, enumerable: true }),
    Object.assign({}, { [Symbol("hidden")]: 7 }),
    Object.assign([7], { [Symbol("hidden")]: 7 }),
    Object.create(null),
    trailingHole,
  ])
    assert.throws(() => snapshot(value));
  for (const value of [
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
  ])
    assert.throws(() => snapshot(value), /accessor/);
});

test("snapshot output defines own data properties without invoking inherited setters", () => {
  const object = runInNewContext(`
    const snapshot = (${snapshot.toString()});
    Object.defineProperty(Object.prototype, "inherited", {set() {throw new Error("inherited object setter invoked");}});
    JSON.stringify(snapshot(JSON.parse('{"inherited":7}')));
  `);
  assert.deepEqual(JSON.parse(object), {
    type: "object",
    value: { inherited: { type: "number", value: "7" } },
  });
  const array = runInNewContext(`
    const snapshot = (${snapshot.toString()});
    Object.defineProperty(Array.prototype, "0", {set() {throw new Error("inherited array setter invoked");}});
    JSON.stringify(snapshot([7]));
  `);
  assert.deepEqual(JSON.parse(array), {
    type: "array",
    value: [{ type: "number", value: "7" }],
  });
});
