import { readFile } from "node:fs/promises";
import type { PortableCommand } from "../../../packages/idealis/src/contracts.js";

export const startWasmText = async (path: string) => {
  const module = await WebAssembly.compile(
    new Uint8Array(await readFile(path)),
  );
  const instance = await WebAssembly.instantiate(module);
  const exports = instance.exports as Record<string, WebAssembly.ExportValue>;
  const memory = exports.memory as WebAssembly.Memory;
  return {
    requestText: (command: PortableCommand): string => {
      const input = new TextEncoder().encode(JSON.stringify(command));
      const pointer =
        (exports.modelt_input_resize as (length: number) => number)(
          input.length,
        ) >>> 0;
      new Uint8Array(memory.buffer, pointer, input.length).set(input);
      (exports.modelt_dispatch as () => void)();
      return new TextDecoder().decode(
        new Uint8Array(
          memory.buffer,
          (exports.modelt_output_ptr as () => number)() >>> 0,
          (exports.modelt_output_len as () => number)() >>> 0,
        ),
      );
    },
  };
};
