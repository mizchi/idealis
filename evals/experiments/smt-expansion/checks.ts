import { readFile } from "node:fs/promises";
import { wasmPath } from "../smt-subset/paths.js";
import { createWasm } from "../smt-subset/wasm.js";
import { generated, handCases } from "./corpus.js";
import { verify } from "./verify.js";

export const checks = async (): Promise<void> => {
  await verify(
    [...handCases(), ...generated(0x4001, 24)],
    0x4001,
    await createWasm(await readFile(wasmPath)),
    () => {},
  );
};
