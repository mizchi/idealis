import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import {
  fixtures,
  observationMatrix,
  requestFor,
} from "../../evals/experiments/roracle-native/fixtures.js";
import {
  encodeBatch,
  encodeBuffer,
} from "../../evals/experiments/roracle-wit/encode.js";
import { createTypedClient } from "../../evals/experiments/roracle-wit/node.js";
import { createOracle } from "../../evals/experiments/workspace-oracle/wasm.js";
import type { JsonValue } from "../../packages/idealis/src/generated/roracle.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

const oracle = createOracle(
  await readFile(
    "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
  ),
);
const clients = await Promise.all(
  (["native-buffer", "wit-record", "wit-buffer"] as const).map((mode) =>
    createTypedClient(mode),
  ),
);

export const registerTypedContract = (
  test: (name: string, run: () => void | Promise<void>) => unknown,
): void => {
  test("typed transports keep all 64 observations and complete reports", () => {
    for (const reference of observationMatrix)
      for (const actual of observationMatrix) {
        const input = requestFor(reference, actual);
        const expected = oracle.request(input);
        for (const client of clients)
          assert.deepEqual(client.request(input), expected);
      }
  });
  test("typed transports keep six fixtures and key order independent values", () => {
    for (const fixture of fixtures) {
      const expected = oracle.request(fixture.input);
      assert.equal(expected.status, fixture.expected);
      for (const client of clients)
        assert.deepEqual(client.request(fixture.input), expected);
    }
    const input = requestFor(
      {
        kind: "value",
        value: JSON.parse('{"__proto__":null,"😀":1,"\ue000":2}'),
      },
      {
        kind: "value",
        value: JSON.parse('{"\ue000":2,"😀":1,"__proto__":null}'),
      },
    );
    assert.equal(oracle.request(input).status, "matched");
    for (const client of clients)
      assert.deepEqual(client.request(input), oracle.request(input));
  });
  test("typed inputs reject bad fields, accessors, scalar coercions and dimensions", () => {
    for (const value of [-0, NaN, Infinity, "\ud800"])
      for (const client of clients)
        assert.throws(() =>
          client.request(
            requestFor({ kind: "value", value }, { kind: "value", value }),
          ),
        );
    for (const client of clients) {
      const input = requestFor(
        { kind: "value", value: 7 },
        { kind: "value", value: 7 },
      );
      assert.throws(() => client.request({ ...input, version: 2 }));
      assert.throws(() => client.request({ ...input, version: 0x100000001 }));
      assert.throws(() => client.request({ ...input, cases: [] }));
      assert.throws(() => client.request({ ...input, implementations: [] }));
      Object.defineProperty(input, "extra", { value: 7, enumerable: true });
      assert.throws(() => client.request(input));
      const getter = requestFor(
        { kind: "value", value: 7 },
        { kind: "value", value: 7 },
      );
      let called = false;
      Object.defineProperty(getter, "group", {
        enumerable: true,
        get: () => {
          called = true;
          return "getter";
        },
      });
      assert.throws(() => client.request(getter));
      assert.equal(called, false);
    }
  });
  test("raw typed native buffer rejects truncation, trailing bytes and bad tags", () => {
    const binding: unknown = createRequire(import.meta.url)(
      resolve("evals/experiments/roracle-wit/generated/roracle-typed.node"),
    );
    assert.ok(isRecord(binding) && typeof binding.checkBuffer === "function");
    const fixture = fixtures[0];
    assert.ok(fixture);
    const batch = encodeBatch(fixture.input);
    const valid = encodeBuffer(batch);
    for (let length = 0; length < valid.length; length++) {
      const result: unknown = binding.checkBuffer(
        Buffer.from(valid.slice(0, length)),
      );
      assert.equal(JSON.parse(String(result)).kind, "error");
    }
    const extra = new Uint8Array(valid.length + 1);
    extra.set(valid);
    assert.equal(
      JSON.parse(binding.checkBuffer(Buffer.from(extra))).kind,
      "error",
    );
    const broken = { ...batch, payload: batch.payload.slice() };
    broken.payload[4] = 99;
    assert.equal(
      JSON.parse(binding.checkBuffer(Buffer.from(encodeBuffer(broken)))).kind,
      "error",
    );
  });
  test("typed number boundaries and deterministic generated values match JSON", () => {
    let seed = 0x6f726163;
    const random = () => {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      return seed;
    };
    const numeric = [
      0,
      0.1,
      Number.MIN_VALUE,
      Number.MAX_VALUE,
      Number.MAX_SAFE_INTEGER,
      2 ** 32,
      -(2 ** 32),
      1e18,
    ];
    const bits = new DataView(new ArrayBuffer(8));
    for (let index = 0; index < 200; index++) {
      bits.setUint32(0, random());
      bits.setUint32(4, random());
      const number = bits.getFloat64(0);
      if (Number.isFinite(number) && !Object.is(number, -0))
        numeric.push(number);
    }
    for (const number of numeric) {
      const value: JsonValue = {
        number,
        "雪😀": [true, null, number],
        empty: {},
        text: '\u0000"\\\n',
      };
      const input = requestFor(
        { kind: "allowed", values: [value, null] },
        { kind: "value", value },
      );
      const expected = oracle.request(input);
      assert.equal(expected.status, "matched");
      for (const client of clients)
        assert.deepEqual(client.request(input), expected);
    }
  });
  test("typed depth boundaries agree with the JSON parser in input, value and allowed", () => {
    let accepted = 0;
    let rejected = 0;
    for (const location of ["input", "value", "allowed"])
      for (let depth = 0; depth < 134; depth++) {
        let value: JsonValue = null;
        for (let index = 0; index < depth; index++) value = [value];
        const input = requestFor(
          location === "allowed"
            ? { kind: "allowed", values: [value] }
            : { kind: "value", value },
          { kind: "value", value },
        );
        if (location === "input") {
          const first = input.cases[0];
          assert.ok(first);
          first.input = value;
          first.reference = { kind: "value", value: 7 };
          first.actual = [{ kind: "value", value: 7 }];
        }
        let expected: ReturnType<typeof oracle.request> | undefined;
        try {
          expected = oracle.request(input);
        } catch (error) {
          assert.ok(error instanceof Error);
          assert.match(error.message, /Oracle batch failed/);
        }
        for (const client of clients) {
          if (expected) assert.deepEqual(client.request(input), expected);
          else assert.throws(() => client.request(input));
        }
        if (expected) accepted++;
        else rejected++;
      }
    assert.ok(accepted > 0 && rejected > 0);
  });
  test("typed size bound rejects the same oversized canonical observation batch", () => {
    const value = "x".repeat(8 * 1024 * 1024);
    const input = requestFor(
      { kind: "value", value },
      { kind: "value", value },
    );
    assert.throws(() => oracle.request(input));
    for (const client of clients) assert.throws(() => client.request(input));
  });
};
