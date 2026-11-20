import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { differences } from "../tlc-differential/compare.js";
import { compareOracle } from "./node.js";

const saved = process.argv[2];
const previous = process.argv[3];
const output = process.argv[4];
assert.ok(saved && previous && output);
const legacy: { differences: typeof differences } = await import(
  pathToFileURL(resolve(previous)).href
);
const rows = JSON.parse(await readFile(resolve(saved, "reports.json"), "utf8"));
const compared = [];
for (const row of rows) {
  const reference = {
    ...row.reference,
    status: ["model-error", "input-error", "infrastructure-error"].includes(
      row.reference.status,
    )
      ? "error"
      : row.reference.status,
    log: await readFile(
      resolve(saved, row.input.name, "model/tlc.log"),
      "utf8",
    ),
  };
  const expected = legacy.differences(row.ours, reference);
  const actual = differences(row.ours, reference);
  assert.deepEqual(actual, expected, row.input.name);
  if (row.native)
    assert.equal(
      compareOracle(row.native, row.ours).status === "matched",
      isDeepStrictEqual(row.native, row.ours),
      row.input.name,
    );
  compared.push({
    name: row.input.name,
    differences: actual,
    originalDifferences: row.differences,
  });
}
await mkdir(output);
await writeFile(
  resolve(output, "result.json"),
  JSON.stringify({
    status: "complete",
    cases: compared.length,
    legacyBehaviorChanged: 0,
    preservedMismatches: rows.filter(
      (row: { differences: readonly unknown[] }) => row.differences.length > 0,
    ).length,
    compared,
  }),
  { flag: "wx" },
);
console.log(
  `${compared.length} legacy comparisons preserved; mismatch reports remain explicit`,
);
