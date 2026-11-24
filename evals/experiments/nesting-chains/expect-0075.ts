import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import type { Baseline, ChainCase } from "./cases.js";
import { addedCases, updatedCases } from "./expectations-0075.js";

const [
  casesPath,
  before0074Path,
  oracle0074Path,
  before0075Path,
  oracle0075Path,
  recordsOutput,
  casesOutput,
  cases0075Output,
] = process.argv.slice(2);
assert.ok(
  casesPath &&
    before0074Path &&
    oracle0074Path &&
    before0075Path &&
    oracle0075Path &&
    recordsOutput &&
    casesOutput &&
    cases0075Output,
  "cases before-0074 oracle-0074 before-0075 oracle-0075 records-output cases-output cases-0075-output",
);
const read = async <T>(path: string): Promise<T> =>
  JSON.parse(await readFile(path, "utf8")) as T;
const records = {
  before0074: await read<Baseline>(before0074Path),
  oracle0074: await read<Baseline>(oracle0074Path),
  before0075: await read<Baseline>(before0075Path),
  oracle0075: await read<Baseline>(oracle0075Path),
};
const cases = await read<readonly ChainCase[]>(casesPath);
const json = (value: unknown) => `${JSON.stringify(value, null, 2)}\n`;
await writeFile(recordsOutput, json(records), { flag: "wx" });
await writeFile(casesOutput, json(updatedCases(cases, records)), {
  flag: "wx",
});
await writeFile(cases0075Output, json(addedCases(records)), { flag: "wx" });
