import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import type { Baseline } from "../nesting-chains/cases.js";
import { depthCases } from "./expectations-0119.js";

const [beforePath, oraclePath, recordsOutput, casesOutput] =
  process.argv.slice(2);
assert.ok(
  beforePath && oraclePath && recordsOutput && casesOutput,
  "before oracle records-output cases-output",
);
const read = async (path: string): Promise<Baseline> =>
  JSON.parse(await readFile(path, "utf8")) as Baseline;
const records = {
  before: await read(beforePath),
  oracle: await read(oraclePath),
};
const json = (value: unknown) => `${JSON.stringify(value, null, 2)}\n`;
await writeFile(recordsOutput, json(records), { flag: "wx" });
await writeFile(casesOutput, json(depthCases(records)), { flag: "wx" });
