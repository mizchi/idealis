import { decodeOracleReply } from "./protocol.js";

export type { OracleReport } from "./protocol.js";

export const createOracle = (bytes: Uint8Array) => {
  const module = new WebAssembly.Module(new Uint8Array(bytes).buffer);
  if (WebAssembly.Module.imports(module).length > 0)
    throw new Error("Oracle Wasm must not need host imports");
  const instance = new WebAssembly.Instance(module, {});
  const memory = instance.exports.memory;
  const version = instance.exports.oracle_abi_version;
  const resize = instance.exports.oracle_input_resize;
  const dispatch = instance.exports.oracle_dispatch;
  const outputPointer = instance.exports.oracle_output_ptr;
  const outputLength = instance.exports.oracle_output_len;
  if (
    !(memory instanceof WebAssembly.Memory) ||
    typeof version !== "function" ||
    typeof resize !== "function" ||
    typeof dispatch !== "function" ||
    typeof outputPointer !== "function" ||
    typeof outputLength !== "function" ||
    version() !== 1
  )
    throw new Error("Unsupported oracle Wasm ABI");
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const requestJson = (json: string) => {
    const data = encoder.encode(json);
    if (data.length > 16 * 1024 * 1024)
      throw new Error("Oracle request exceeds 16 MiB");
    const pointer = resize(data.length);
    if (!Number.isInteger(pointer) || pointer <= 0)
      throw new Error("Invalid oracle input pointer");
    new Uint8Array(memory.buffer, pointer, data.length).set(data);
    dispatch();
    const start = outputPointer();
    const length = outputLength();
    if (
      !Number.isInteger(start) ||
      !Number.isInteger(length) ||
      start < 0 ||
      length < 0
    )
      throw new Error("Invalid oracle output buffer");
    return decodeOracleReply(
      decoder.decode(new Uint8Array(memory.buffer, start, length)),
    );
  };
  return {
    requestJson,
    request: (input: unknown) => requestJson(JSON.stringify(input)),
  };
};
