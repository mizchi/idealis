import { readFile } from "node:fs/promises";
import { generated, handCases } from "../smt-scripts/corpus.js";
import { wasmPath } from "../smt-subset/paths.js";
import { createWasm } from "../smt-subset/wasm.js";
import { verify } from "./verify.js";
export const checks = async (): Promise<void> => {
  await verify(
    [...handCases(), ...generated(0x4201, 4)],
    await createWasm(await readFile(wasmPath)),
    () => {},
  );
};
