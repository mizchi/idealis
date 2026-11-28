import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import type { Baseline } from "../nesting-chains/cases.js";
import { depthCases0120 } from "./expectations-0120.js";

const [beforePath, recordsOutput, casesOutput] = process.argv.slice(2);
assert.ok(
  beforePath && recordsOutput && casesOutput,
  "before records-output cases-output",
);
const records = {
  before: JSON.parse(await readFile(beforePath, "utf8")) as Baseline,
};
const json = (value: unknown) => `${JSON.stringify(value, null, 2)}\n`;
await writeFile(recordsOutput, json(records), { flag: "wx" });
await writeFile(casesOutput, json(depthCases0120(records)), { flag: "wx" });
