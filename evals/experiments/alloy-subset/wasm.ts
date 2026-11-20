import { decode, type Reply, type Request } from "./wire.js";

export const createWasm = async (
  bytes: Uint8Array,
): Promise<{ request: (request: Request) => Reply }> => {
  const module = await WebAssembly.compile(new Uint8Array(bytes).buffer);
  if (WebAssembly.Module.imports(module).length)
    throw new Error("ralloy must not import host functions");
  const instance = await WebAssembly.instantiate(module);
  const memory = instance.exports.memory;
  if (!(memory instanceof WebAssembly.Memory))
    throw new Error("Missing ralloy memory");
  const method = (name: string): ((...args: number[]) => number) => {
    const value = instance.exports[`ralloy_${name}`];
    if (typeof value !== "function") throw new Error(`Missing ralloy ${name}`);
    return (...args) => Number(value(...args));
  };
  if (method("abi_version")() !== 1) throw new Error("Invalid ralloy ABI");
  return {
    request: (request) => {
      const input = new TextEncoder().encode(
        JSON.stringify({ version: 1, request }),
      );
      const pointer = method("input_resize")(input.length);
      new Uint8Array(memory.buffer, pointer, input.length).set(input);
      method("dispatch")();
      const output = new Uint8Array(
        memory.buffer,
        method("output_ptr")(),
        method("output_len")(),
      );
      return decode(JSON.parse(new TextDecoder().decode(output)));
    },
  };
};
