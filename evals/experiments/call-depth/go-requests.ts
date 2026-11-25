import assert from "node:assert/strict";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { hopSource } from "./wasm-ceiling.js";

const [directory, ...levels] = process.argv.slice(2);
assert.ok(
  directory && levels.length > 0,
  "usage: go-requests.js <dir> <levels>...",
);
for (const count of levels)
  writeFileSync(
    join(directory, `hop-${count.padStart(4, "0")}.json`),
    JSON.stringify({
      version: 1,
      request: {
        kind: "check-tla",
        source: hopSource(Number(count)),
        config: "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
        bounds: { maxDepth: 64, maxStates: 10000, maxWork: 100_000_000 },
        irVersion: 2,
      },
    }),
    { flag: "wx" },
  );
