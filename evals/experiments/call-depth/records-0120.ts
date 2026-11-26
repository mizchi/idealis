import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import type { Baseline } from "../nesting-chains/cases.js";
import { depthCases0120, type Records0120 } from "./expectations-0120.js";

const [beforePath, oraclePath, oracleAfterPath, recordsOutput, casesOutput] =
  process.argv.slice(2);
assert.ok(
  beforePath && oraclePath && oracleAfterPath && recordsOutput && casesOutput,
  "before oracle oracle-after records-output cases-output",
);
const read = async (path: string): Promise<Baseline> =>
  JSON.parse(await readFile(path, "utf8")) as Baseline;
const records: Records0120 & { oracle: Baseline; oracleAfter: Baseline } = {
  before: await read(beforePath),
  oracle: await read(oraclePath),
  oracleAfter: await read(oracleAfterPath),
};
const json = (value: unknown) => `${JSON.stringify(value, null, 2)}\n`;
await writeFile(recordsOutput, json(records), { flag: "wx" });
await writeFile(casesOutput, json(depthCases0120(records)), { flag: "wx" });
