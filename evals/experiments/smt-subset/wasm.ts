import type { Reply, Request } from "./wire.js";
import { decode } from "./wire.js";
export const createWasm = async (
  bytes: Uint8Array,
): Promise<{ request: (request: Request) => Reply }> => {
  const module = await WebAssembly.compile(Uint8Array.from(bytes));
  if (WebAssembly.Module.imports(module).length)
    throw new Error("SMT Wasm must have no host imports");
  const exports = (await WebAssembly.instantiate(module)).exports;
  if (!(exports.memory instanceof WebAssembly.Memory))
    throw new Error("Missing SMT memory");
  const memory = exports.memory;
  const call = (name: string, ...args: number[]): number => {
    const method = exports[`smt_${name}`];
    if (typeof method !== "function") throw new Error(`Missing SMT ${name}`);
    return Number(method(...args));
  };
  if (call("abi_version") !== 1) throw new Error("Unsupported SMT ABI");
  return {
    request: (request) => {
      const input = new TextEncoder().encode(
        JSON.stringify({ version: 1, request }),
      );
      const pointer = call("input_resize", input.length);
      new Uint8Array(memory.buffer, pointer, input.length).set(input);
      call("dispatch");
      return decode(
        JSON.parse(
          new TextDecoder("utf-8", { fatal: true }).decode(
            new Uint8Array(
              memory.buffer,
              call("output_ptr"),
              call("output_len"),
            ).slice(),
          ),
        ),
      );
    },
  };
};
