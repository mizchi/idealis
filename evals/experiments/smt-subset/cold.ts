import { readFile } from "node:fs/promises";
import type { Input } from "./corpus.js";
import { maxWork, wasmPath } from "./paths.js";
import { createWasm } from "./wasm.js";
import { solution } from "./wire.js";

const input: Input = JSON.parse(await readFile(process.argv[3] ?? "", "utf8"));
if (process.argv[2] === "rust-wasm") {
  const core = await createWasm(await readFile(wasmPath));
  console.log(
    JSON.stringify(
      solution(core.request({ kind: "solve", source: input.source, maxWork })),
    ),
  );
} else {
  const { createZ3 } = await import("./clients.js");
  const z3 = await createZ3();
  try {
    console.log(JSON.stringify(await z3.solve(input)));
  } finally {
    await z3.close();
  }
}
