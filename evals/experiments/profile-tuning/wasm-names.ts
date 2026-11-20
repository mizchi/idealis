import assert from "node:assert/strict";
import { digest } from "../usable/provenance.js";

interface Section {
  readonly id: number;
  readonly bytes: Uint8Array<ArrayBuffer>;
  readonly payload: Uint8Array<ArrayBuffer>;
}
const sections = (bytes: Uint8Array<ArrayBuffer>): readonly Section[] => {
  assert.ok(WebAssembly.validate(bytes), "Invalid Wasm");
  const result: Section[] = [];
  let offset = 8;
  while (offset < bytes.length) {
    const start = offset;
    const id = bytes[offset++];
    assert.ok(id !== undefined);
    let length = 0;
    let shift = 0;
    for (;;) {
      const byte = bytes[offset++];
      assert.ok(byte !== undefined && shift <= 28, "Invalid section length");
      length += (byte & 127) * 2 ** shift;
      if (byte < 128) break;
      shift += 7;
    }
    const end = offset + length;
    assert.ok(end <= bytes.length, "Truncated section");
    result.push({
      id,
      bytes: bytes.subarray(start, end),
      payload: bytes.subarray(offset, end),
    });
    offset = end;
  }
  return result;
};
export const executableDigest = (bytes: Uint8Array<ArrayBuffer>): string =>
  digest(
    Buffer.concat(
      sections(bytes)
        .filter((section) => section.id !== 0)
        .map((section) => section.bytes),
    ),
  );
export const withNames = (
  production: Uint8Array<ArrayBuffer>,
  symbols: Uint8Array<ArrayBuffer>,
): Uint8Array<ArrayBuffer> => {
  const original = sections(production);
  const named = sections(symbols);
  for (const id of [1, 2, 3, 10]) {
    assert.deepEqual(
      original.find((section) => section.id === id)?.payload,
      named.find((section) => section.id === id)?.payload,
      "Function code differs",
    );
  }
  const name = named.find(
    (section) =>
      section.id === 0 &&
      Buffer.from(section.payload.subarray(0, 5)).equals(
        Buffer.from([4, 110, 97, 109, 101]),
      ),
  );
  assert.ok(name, "Missing name section");
  const result = new Uint8Array(production.length + name.bytes.length);
  result.set(production);
  result.set(name.bytes, production.length);
  assert.equal(executableDigest(result), executableDigest(production));
  return result;
};
