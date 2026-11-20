import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { readOracleWasm } from "./wasm.js";

const before = process.argv[2];
const after = process.argv[3];
const output = process.argv[4];
assert.ok(before && after && output);
await mkdir(output);
const values = [
  { kind: "value", value: { nested: [null, true, 7, "日本語", { a: 1 }] } },
  { kind: "value", value: -1 },
  { kind: "allowed", values: [1, 2] },
  { kind: "allowed", values: [] },
  { kind: "unknown", reason: "budget" },
  { kind: "invalid", reason: "input" },
  { kind: "error", message: "crash" },
];
const request = (reference: unknown, actual: unknown) => ({
  version: 1,
  group: "compat",
  reference: "oracle",
  implementations: ["target"],
  cases: [
    {
      id: "one",
      input: { retained: [1, null, "x"] },
      reference,
      actual: [actual],
    },
  ],
});
const inputs = values.flatMap((reference) =>
  values.map((actual) => JSON.stringify(request(reference, actual))),
);
inputs.push(
  "invalid",
  "{}",
  JSON.stringify({ ...request(values[0], values[0]), cases: [] }),
  JSON.stringify({ ...request(values[0], values[0]), version: 2 }),
);
// Alternating large, malformed and small requests exercises reused output buffers.
inputs.push(
  JSON.stringify(
    request(
      {
        kind: "value",
        value: Array.from({ length: 4096 }, (_, index) => ({
          index,
          text: "日本語",
        })),
      },
      values[0],
    ),
  ),
  "invalid",
  JSON.stringify(request(values[0], values[0])),
);
const paths = [];
const replies: unknown[][] = [];
for (const root of [before, after]) {
  const native = resolve(root, "oracle");
  const bytes = await readOracleWasm(root);
  const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes), {});
  const memory = instance.exports.memory;
  const resize = instance.exports.oracle_input_resize;
  const dispatch = instance.exports.oracle_dispatch;
  const ptr = instance.exports.oracle_output_ptr;
  const len = instance.exports.oracle_output_len;
  assert.ok(
    memory instanceof WebAssembly.Memory &&
      typeof resize === "function" &&
      typeof dispatch === "function" &&
      typeof ptr === "function" &&
      typeof len === "function",
  );
  const wasm = inputs.map((input) => {
    const encoded = new TextEncoder().encode(input);
    const pointer = resize(encoded.length);
    new Uint8Array(memory.buffer, pointer, encoded.length).set(encoded);
    dispatch();
    return JSON.parse(
      new TextDecoder().decode(new Uint8Array(memory.buffer, ptr(), len())),
    );
  });
  const nativeReplies = execFileSync(native, [], {
    input: `${inputs.join("\n")}\n`,
    maxBuffer: 32 * 1024 * 1024,
  })
    .toString()
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  assert.deepEqual(wasm, nativeReplies);
  replies.push(nativeReplies);
  paths.push({
    root: resolve(root),
    nativeSha256: createHash("sha256")
      .update(await readFile(native))
      .digest("hex"),
    wasmSha256: createHash("sha256").update(bytes).digest("hex"),
  });
}
assert.deepEqual(replies[0], replies[1]);
await writeFile(
  resolve(output, "result.json"),
  `${JSON.stringify({ status: "matched", cases: inputs.length, paths, inputs, replies }, null, 2)}\n`,
  { flag: "wx" },
);
console.log(
  `${inputs.length} complete responses matched across old/new native/Wasm`,
);
