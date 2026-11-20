export const createWasm = async (bytes: Uint8Array) => {
  const module = await WebAssembly.compile(new Uint8Array(bytes));
  const imports = WebAssembly.Module.imports(module);
  if (imports.length)
    throw new Error("Testing core must not have host imports");
  const instance = await WebAssembly.instantiate(module, {});
  const memory = instance.exports.memory;
  if (!(memory instanceof WebAssembly.Memory))
    throw new Error("Missing testing memory");
  const call = (name: string, ...args: number[]) => {
    const method = instance.exports[`testing_${name}`];
    if (typeof method !== "function")
      throw new Error(`Missing testing ${name}`);
    return Number(method(...args));
  };
  if (call("abi_version") !== 1) throw new Error("Wrong testing ABI");
  return {
    imports,
    request: (bytes: Uint8Array): unknown => {
      const pointer = call("input_resize", bytes.length);
      new Uint8Array(memory.buffer, pointer, bytes.length).set(bytes);
      call("dispatch");
      return JSON.parse(
        new TextDecoder().decode(
          new Uint8Array(memory.buffer, call("output_ptr"), call("output_len")),
        ),
      );
    },
  };
};
export const equalReply = (native: unknown, wasm: unknown): boolean => {
  if (
    typeof native === "object" &&
    native !== null &&
    "kind" in native &&
    native.kind === "error"
  )
    return (
      typeof wasm === "object" &&
      wasm !== null &&
      "kind" in wasm &&
      wasm.kind === "error" &&
      "error" in wasm &&
      typeof wasm.error === "object" &&
      wasm.error !== null &&
      "kind" in wasm.error &&
      (wasm.error.kind === "protocol" || wasm.error.kind === "resource") &&
      "message" in wasm.error &&
      typeof wasm.error.message === "string"
    );
  const canonical = (value: unknown): unknown =>
    Array.isArray(value)
      ? value.map(canonical)
      : typeof value === "object" && value !== null
        ? Object.fromEntries(
            Object.entries(value)
              .sort(([left], [right]) => left.localeCompare(right))
              .map(([key, entry]) => [key, canonical(entry)]),
          )
        : value;
  return JSON.stringify(canonical(native)) === JSON.stringify(canonical(wasm));
};
