import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";

// threshold <development-run> <confirmation-run> <output>
// Rule fixed in evals/plans/0087-ralloy-backend-threshold.md.
const [development, confirmation, output] = process.argv.slice(2);
assert.ok(development && confirmation && output);
type Record = {
  group: string;
  model: string;
  command: string;
  width: number;
  enumerateNs: number | null;
  satNs: number;
};
const load = async (path: string): Promise<Record[]> =>
  (await readFile(join(path, "records.jsonl"), "utf8"))
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
const candidates = [
  ...Array.from({ length: 13 }, (_, index) => index * 4),
  Number.POSITIVE_INFINITY,
];
const total = (records: readonly Record[], threshold: number) =>
  records.reduce(
    (sum, record) =>
      sum +
      (record.width > threshold
        ? record.satNs
        : (record.enumerateNs ?? Number.POSITIVE_INFINITY)),
    0,
  );
const [developmentRecords, confirmationRecords] = await Promise.all([
  load(development),
  load(confirmation),
]);
const rows = candidates.map((threshold) => ({
  threshold: Number.isFinite(threshold) ? threshold : "never",
  developmentNs: total(developmentRecords, threshold),
  confirmationNs: total(confirmationRecords, threshold),
}));
// Least development total; ties prefer the larger threshold.
let chosen = 0;
for (const [index, row] of rows.entries())
  if (row.developmentNs <= rows[chosen]!.developmentNs) chosen = index;
const best = Math.min(...rows.map((row) => row.confirmationNs));
const confirmed = rows[chosen]!.confirmationNs <= best * 1.2;
const result = {
  status: "complete",
  plan: "0087-ralloy-backend-threshold",
  chosen: rows[chosen]!.threshold,
  decision: confirmed ? "adopt" : "hold",
  confirmationBestNs: best,
  rows,
  runs: [development, confirmation],
};
await mkdir(dirname(output), { recursive: true });
await mkdir(output);
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify(result, null, 2)}\n`,
  {
    flag: "wx",
  },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(output), kind: "ralloy-backend-threshold", plan: "0087-ralloy-backend-threshold", status: "complete", decision: result.decision })}\n`,
);
console.log(
  JSON.stringify({ chosen: result.chosen, decision: result.decision }),
);
