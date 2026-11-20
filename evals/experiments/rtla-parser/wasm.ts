export const createParserWasm = async (bytes: Uint8Array) => {
  const module = await WebAssembly.compile(Uint8Array.from(bytes));
  if (WebAssembly.Module.imports(module).length)
    throw new Error("rtla must not require host imports");
  const instance = await WebAssembly.instantiate(module, {});
  const memory = instance.exports.memory;
  if (!(memory instanceof WebAssembly.Memory))
    throw new Error("Missing rtla memory");
  const call = (name: string, ...args: number[]): number => {
    const entry = instance.exports[name];
    if (typeof entry !== "function") throw new Error(`Missing ${name}`);
    const result: unknown = entry(...args);
    if (
      typeof result !== "number" ||
      !Number.isSafeInteger(result) ||
      result < 0
    )
      throw new Error(`Invalid result from ${name}`);
    return result;
  };
  if (call("rtla_abi_version") !== 1) throw new Error("Unknown rtla ABI");
  return {
    request: (request: unknown): unknown => {
      const input = new TextEncoder().encode(JSON.stringify(request));
      const pointer = call("rtla_input_resize", input.length);
      new Uint8Array(memory.buffer, pointer, input.length).set(input);
      const dispatch = instance.exports.rtla_dispatch;
      if (typeof dispatch !== "function") throw new Error("Missing dispatch");
      dispatch();
      const output = new Uint8Array(
        memory.buffer,
        call("rtla_output_ptr"),
        call("rtla_output_len"),
      );
      return JSON.parse(
        new TextDecoder("utf-8", { fatal: true }).decode(output),
      );
    },
  };
};
