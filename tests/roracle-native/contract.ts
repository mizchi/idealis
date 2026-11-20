import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { canonicalObject } from "../../evals/experiments/roracle-native/canonical.js";
import {
  createNativeOracle,
  loadNativeBinding,
} from "../../evals/experiments/roracle-native/client.js";
import {
  fixtures,
  observationMatrix,
  requestFor,
} from "../../evals/experiments/roracle-native/fixtures.js";
import { summarizeObject } from "../../evals/experiments/roracle-native/object-summary.js";
import { decodeOracleReply } from "../../evals/experiments/workspace-oracle/protocol.js";
import { snapshot } from "../../evals/experiments/workspace-oracle/snapshot.js";
import { createOracle } from "../../evals/experiments/workspace-oracle/wasm.js";
import type { JsonValue } from "../../packages/idealis/src/generated/roracle.js";

const bytes = await readFile(
  "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
);
const wasm = createOracle(bytes);
const native = [
  createNativeOracle("native-json"),
  createNativeOracle("native-object"),
];

export const registerNativeOracleContract = (
  test: (name: string, run: () => void | Promise<void>) => unknown,
): void => {
  test("object speed comparison refuses empty suites and missing samples", () => {
    assert.throws(() => summarizeObject([], []));
    const fixture = fixtures[0];
    assert.ok(fixture);
    assert.throws(() =>
      summarizeObject(
        [
          {
            name: fixture.name,
            iterations: fixture.iterations,
            input: fixture.input,
            expected: wasm.request(fixture.input),
          },
        ],
        [],
      ),
    );
  });
  test("native and Wasm preserve all 64 observation combinations and complete reports", () => {
    for (const reference of observationMatrix)
      for (const actual of observationMatrix) {
        const request = requestFor(reference, actual);
        const expected = wasm.request(request);
        for (const client of native)
          assert.deepEqual(client.request(request), expected);
      }
  });
  test("transport fixtures retain positive, mismatch and unknown controls", () => {
    for (const fixture of fixtures) {
      const expected = wasm.request(fixture.input);
      assert.equal(expected.status, fixture.expected);
      for (const client of native)
        assert.deepEqual(client.request(fixture.input), expected);
    }
  });
  test("special keys and explicit JS snapshots survive both transports", () => {
    const value = snapshot(
      JSON.parse('{"__proto__":{"kept":7},"constructor":null,"雪":"😀"}'),
    );
    const input = requestFor(
      { kind: "value", value },
      { kind: "value", value },
    );
    const expected = wasm.request(input);
    for (const client of native)
      assert.deepEqual(client.request(input), expected);
  });
  test("finite numeric boundaries and shared data retain complete reports", () => {
    for (const value of [
      0,
      0.1,
      -1.5,
      2 ** 32,
      -(2 ** 32),
      Number.MAX_SAFE_INTEGER,
      Number.MAX_VALUE,
      Number.MIN_VALUE,
    ]) {
      const input = requestFor(
        { kind: "value", value },
        { kind: "value", value },
      );
      const expected = wasm.request(input);
      assert.equal(expected.status, "matched");
      for (const client of native)
        assert.deepEqual(client.request(input), expected);
    }
    const shared = { nested: [1, null, "雪"] };
    const value = { first: shared, second: shared };
    const input = requestFor(
      { kind: "value", value },
      { kind: "value", value },
    );
    for (const client of native)
      assert.deepEqual(client.request(input), wasm.request(input));
  });
  test("native raw JSON keeps invalid input, duplicate fields and dimension failures rejected", () => {
    const binding = loadNativeBinding();
    const valid = requestFor(
      { kind: "value", value: 7 },
      { kind: "value", value: 7 },
    );
    for (const json of [
      "invalid",
      "{}",
      JSON.stringify({ ...valid, version: 2 }),
      JSON.stringify({ ...valid, cases: [] }),
      JSON.stringify(valid).replace('"value":7', '"value":7,"value":8'),
    ]) {
      assert.throws(() => decodeOracleReply(binding.dispatchJson(json)));
      assert.throws(() => wasm.requestJson(json));
    }
  });
  test("native object rejects unsafe coercions and accessors before conversion", () => {
    const client = native[1];
    assert.ok(client);
    for (const value of [-0, NaN, Infinity, "\ud800"]) {
      const request = requestFor(
        { kind: "value", value },
        { kind: "value", value },
      );
      assert.throws(() => client.request(request));
    }
    const request = requestFor(
      { kind: "value", value: 7 },
      { kind: "value", value: 7 },
    );
    let called = false;
    Object.defineProperty(request, "group", {
      get: () => {
        called = true;
        return "getter";
      },
      enumerable: true,
    });
    assert.throws(() => client.request(request), /accessors/);
    assert.equal(called, false);
  });
  test("native object rejects cyclic, sparse and inherited data", () => {
    const client = native[1];
    assert.ok(client);
    const cycle: { nested?: unknown } = {};
    cycle.nested = cycle;
    const sparse: unknown[] = [];
    sparse.length = 2;
    for (const invalid of [
      cycle,
      sparse,
      new Date(),
      Object.create({ inherited: 7 }),
    ]) {
      const request = requestFor(
        { kind: "value", value: null },
        { kind: "value", value: null },
      );
      Object.defineProperty(request.cases[0]?.reference, "value", {
        value: invalid,
      });
      assert.throws(() => client.request(request));
    }
    execFileSync(
      process.execPath,
      [
        "--input-type=module",
        "-e",
        `
      import assert from 'node:assert/strict';
      import { createNativeOracle } from './dist/evals/experiments/roracle-native/client.js';
      import { fixtures } from './dist/evals/experiments/roracle-native/fixtures.js';
      const client = createNativeOracle('native-object');
      Object.defineProperty(Object.prototype, 'inherited', { enumerable: true, get() { throw new Error('getter ran'); } });
      assert.throws(() => client.request(fixtures[0].input), /inherited enumerable/);
    `,
      ],
      { stdio: "pipe" },
    );
  });
  test("object validation rechecks mutations, hidden fields and symbol keys on every call", () => {
    const value = { changed: 7 };
    canonicalObject(value);
    Object.defineProperty(value, "hidden", { value: 7 });
    assert.throws(() => canonicalObject(value), /hidden/);
    const withSymbol = { [Symbol("ignored")]: 7 };
    assert.throws(() => canonicalObject(withSymbol), /symbol/);
    const extended = [7];
    Object.defineProperty(extended, "extra", { value: 8 });
    assert.throws(() => canonicalObject(extended), /extended/);
    const accessor = [7];
    let called = false;
    Object.defineProperty(accessor, "0", {
      get: () => {
        called = true;
        return 7;
      },
    });
    assert.throws(() => canonicalObject(accessor), /accessors/);
    assert.equal(called, false);
    execFileSync(
      process.execPath,
      [
        "--input-type=module",
        "-e",
        `
      import assert from 'node:assert/strict';
      import { canonicalObject } from './dist/evals/experiments/roracle-native/canonical.js';
      canonicalObject({ stable: 7 });
      let called = false;
      Object.defineProperty(Object.prototype, 'newField', { enumerable: true, get() { called = true; return 7; } });
      assert.throws(() => canonicalObject({ stable: 7 }), /inherited enumerable/);
      assert.equal(called, false);
    `,
      ],
      { stdio: "pipe" },
    );
  });
  test("object validation keeps depth and expanded-node budgets for shared references", () => {
    let value: JsonValue = null;
    for (let index = 0; index < 129; index++) value = [value];
    assert.throws(() => canonicalObject(value), /budget/);
    let shared: JsonValue = 7;
    for (let index = 0; index < 20; index++) shared = [shared, shared];
    assert.throws(() => canonicalObject(shared), /budget/);
    let keyed: JsonValue = 7;
    for (let index = 0; index < 18; index++)
      keyed = { left: keyed, right: keyed };
    assert.throws(() => canonicalObject(keyed), /budget/);
  });
  test("missing native binaries never silently fall back to Wasm", () => {
    assert.throws(() =>
      createNativeOracle(
        "native-json",
        ".artifacts/definitely-missing-roracle.node",
      ),
    );
  });
  test("corrupted full reports fail validation instead of passing on status", () => {
    const reply = JSON.parse(
      loadNativeBinding().dispatchJson(JSON.stringify(fixtures[0]?.input)),
    );
    reply.report.cases[0].comparisons[0].actual = { kind: "value" };
    assert.throws(() => decodeOracleReply(JSON.stringify(reply)));
    delete reply.report.reference;
    assert.throws(() => decodeOracleReply(JSON.stringify(reply)));
    for (const actual of [
      { kind: "value", value: null, extra: true },
      { kind: "allowed", values: null },
      { kind: "unknown", reason: 1 },
      { kind: "invalid" },
      { kind: "error", message: false },
    ]) {
      const broken = JSON.parse(
        loadNativeBinding().dispatchJson(JSON.stringify(fixtures[0]?.input)),
      );
      broken.report.cases[0].comparisons[0].actual = actual;
      assert.throws(() => decodeOracleReply(JSON.stringify(broken)));
    }
    for (const cases of [
      [],
      [
        {
          case: { id: "one", input: null },
          reference: { kind: "value", value: 7 },
          comparisons: [],
        },
      ],
    ]) {
      const broken = JSON.parse(
        loadNativeBinding().dispatchJson(JSON.stringify(fixtures[0]?.input)),
      );
      broken.report.cases = cases;
      assert.throws(() => decodeOracleReply(JSON.stringify(broken)));
    }
  });
  test("Rust-generated contract declarations are current", () => {
    execFileSync(
      process.execPath,
      ["--experimental-strip-types", "scripts/oracle-types.ts", "--check"],
      { stdio: "pipe" },
    );
  });
};
