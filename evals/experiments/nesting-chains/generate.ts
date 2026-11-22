import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { type Baseline, chainCases } from "./cases.js";

const [baselinePath, output] = process.argv.slice(2);
assert.ok(baselinePath && output, "baseline output");
const baseline: Baseline = JSON.parse(await readFile(baselinePath, "utf8"));
await writeFile(output, `${JSON.stringify(chainCases(baseline), null, 2)}\n`, {
  flag: "wx",
});
