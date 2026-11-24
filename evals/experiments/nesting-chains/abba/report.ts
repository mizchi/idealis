import { readFile } from "node:fs/promises";
import { join } from "node:path";
import {
  compareAll,
  readEstimates,
  sampleDirectory,
  sampleSides,
  type SampleRecord,
} from "./samples.js";

const [run, ...expected] = process.argv.slice(2);
if (run === undefined || expected.length === 0) {
  console.error("usage: report.js <run directory> <benchmark>...");
  process.exit(2);
}

const recorded = new Map<
  string,
  {
    records: readonly SampleRecord[];
    estimates: Awaited<ReturnType<typeof readEstimates>>[];
  }
>();
for (const benchmark of expected) {
  const base = join(run, benchmark);
  try {
    const records = JSON.parse(
      await readFile(join(base, "runs.json"), "utf8"),
    ) as SampleRecord[];
    const estimates = await Promise.all(
      sampleSides.map((side, index) =>
        readEstimates(
          join(base, sampleDirectory(index, side), "criterion"),
        ).catch(() => new Map<string, number>()),
      ),
    );
    recorded.set(benchmark, { records, estimates });
  } catch {
    continue;
  }
}

const report = compareAll(expected, recorded);
for (const row of report.comparisons) {
  console.log(
    `${row.name.padEnd(80)}${row.before.toFixed(0).padStart(12)}${row.after
      .toFixed(0)
      .padStart(12)}${row.change.toFixed(2).padStart(8)}%`,
  );
}
const changes = report.comparisons.map((row) => row.change);
console.log(
  `benchmarks ${expected.length - report.incomplete.length}/${expected.length} complete, functions ${changes.length}` +
    (changes.length > 0
      ? `, max change ${Math.max(...changes).toFixed(2)}%, min ${Math.min(...changes).toFixed(2)}%, at or above +10%: ${changes.filter((change) => change >= 10).length}`
      : ""),
);
for (const item of report.incomplete) {
  console.log(`INCOMPLETE ${item.benchmark}: ${item.reason}`);
}
process.exit(report.incomplete.length === 0 ? 0 : 1);
