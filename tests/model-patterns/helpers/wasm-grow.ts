// (module (memory (export "memory") 1 2)
//   (func (export "grow") (param i32) (result i32) local.get 0 memory.grow))
const module = new WebAssembly.Module(
  Uint8Array.from(
    Buffer.from(
      [
        "0061736d01000000",
        "01060160017f017f",
        "03020100",
        "050401010102",
        "071102066d656d6f727902000467726f770000",
        "0a08010600200040000b",
      ].join(""),
      "hex",
    ),
  ),
);

export function openGrowMemory(): {
  memory: WebAssembly.Memory;
  grow: (delta: number) => number;
} {
  const { exports } = new WebAssembly.Instance(module);
  return {
    memory: exports.memory as WebAssembly.Memory,
    grow: exports.grow as (delta: number) => number,
  };
}
