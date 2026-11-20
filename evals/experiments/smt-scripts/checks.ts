import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { wasmPath } from "../smt-subset/paths.js";
import { createWasm } from "../smt-subset/wasm.js";
import { decode } from "../smt-subset/wire.js";
import { generated, handCases } from "./corpus.js";
import { verify } from "./verify.js";
export const checks = async (): Promise<void> => {
  await verify(
    [...handCases(), ...generated(0x4101, 4)],
    await createWasm(await readFile(wasmPath)),
    () => {},
  );
  for (const results of [
    null,
    {},
    Array.from({ length: 257 }, () => ({})),
    [
      {
        status: "unknown",
        values: { x: "7" },
        booleans: {},
        reason: "work",
        stats: { work: 0, branches: 0, relaxations: 0 },
      },
    ],
  ])
    assert.throws(() => decode({ kind: "script-results", results }));
};
