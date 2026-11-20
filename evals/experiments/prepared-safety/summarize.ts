import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { gunzipSync } from "node:zlib";
import { digest } from "../usable/provenance.js";

interface Row {
  readonly input: string;
  readonly irVersion: number;
  readonly repetitions: number;
  readonly iteration: number;
  readonly warmup: boolean;
  readonly route: string;
  readonly prepareMs: number;
  readonly checkMs: number;
  readonly releaseMs: number;
  readonly totalMs: number;
  readonly responses: { readonly path: string; readonly sha256: string };
}
interface Cold {
  readonly input: string;
  readonly irVersion: number;
  readonly repetitions: number;
  readonly iteration: number;
  readonly warmup: boolean;
  readonly route: string;
  readonly wallMs: number;
  readonly maxRssBytes: number;
  readonly response: { readonly path: string; readonly sha256: string };
}
interface Result {
  readonly status: string;
  readonly records: readonly Row[];
  readonly cold: readonly Cold[];
  readonly unsupported: readonly unknown[];
}
const [input, output] = process.argv.slice(2);
assert.ok(input && output);
const raw = await readFile(join(input, "result.json"));
const result: Result = JSON.parse(raw.toString());
assert.equal(result.status, "complete");
assert.equal(result.records.length, 2336);
assert.equal(result.cold.length, 1168);
assert.equal(result.unsupported.length, 1);
const unpack = async (reference: {
  readonly path: string;
  readonly sha256: string;
}) => {
  const bytes = await readFile(join(input, reference.path));
  assert.equal(digest(bytes), reference.sha256);
  return JSON.parse(gunzipSync(bytes).toString());
};
const groups = new Map<string, Row[]>();
for (const row of result.records) {
  const key = JSON.stringify([row.input, row.irVersion, row.repetitions]);
  const group = groups.get(key) ?? [];
  group.push(row);
  groups.set(key, group);
}
const stats = (values: readonly number[]) => {
  const sorted = values.toSorted((left, right) => left - right);
  assert.equal(sorted.length, 3);
  assert.ok(sorted.every(Number.isFinite));
  return { min: sorted[0], median: sorted[1], max: sorted[2] };
};
const summary = [];
let checks = 0;
for (const rows of groups.values()) {
  assert.equal(rows.length, 32);
  const initial = rows[0];
  assert.ok(initial);
  const baseline = rows.find(
    (row) => row.route === "before-native" && row.iteration === 0,
  );
  assert.ok(baseline);
  const expected = await unpack(baseline.responses);
  assert.equal(expected.length, initial.repetitions);
  const names = [...new Set(rows.map((row) => row.route))];
  assert.equal(names.length, 8);
  for (const row of rows) {
    assert.deepEqual(await unpack(row.responses), expected);
    checks += initial.repetitions;
  }
  const cold = result.cold.filter(
    (row) =>
      row.input === initial.input &&
      row.irVersion === initial.irVersion &&
      row.repetitions === initial.repetitions,
  );
  assert.equal(cold.length, 16);
  for (const row of cold) {
    const packet = await unpack(row.response);
    assert.equal(packet.status, 0);
    const responses = packet.stdout
      .trim()
      .split("\n")
      .map((line: string) => JSON.parse(line));
    const actual =
      row.route.endsWith("-native") &&
      !row.route.startsWith("batch-") &&
      !row.route.startsWith("reuse-")
        ? responses
        : responses
            .slice(1, -1)
            .flatMap((packet: { kind: string; results?: unknown[] }) =>
              packet.kind === "safety-batch" ? packet.results : [packet],
            );
    assert.deepEqual(actual, expected);
    checks += initial.repetitions;
  }
  summary.push({
    input: initial.input,
    irVersion: initial.irVersion,
    repetitions: initial.repetitions,
    routes: names.map((name) => {
      const records = rows.filter((row) => row.route === name && !row.warmup);
      assert.deepEqual(
        records.map((row) => row.iteration).toSorted(),
        [1, 2, 3],
      );
      const processes = cold.filter((row) => row.route === name && !row.warmup);
      return {
        name,
        prepareMs: stats(records.map((row) => row.prepareMs)),
        checkMs: stats(records.map((row) => row.checkMs)),
        releaseMs: stats(records.map((row) => row.releaseMs)),
        totalMs: stats(records.map((row) => row.totalMs)),
        coldMs: processes.length
          ? stats(processes.map((row) => row.wallMs))
          : null,
        rssBytes: processes.length
          ? stats(processes.map((row) => row.maxRssBytes))
          : null,
      };
    }),
  });
}
assert.equal(groups.size, 73);
await mkdir(output);
await writeFile(
  join(output, "result.json"),
  `${JSON.stringify({ status: "complete", plan: "0064-prepared-safety", input, resultSha256: digest(raw), queriesCompared: checks, groups: groups.size, records: result.records.length, cold: result.cold.length, summary }, null, 2)}\n`,
  { flag: "wx" },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: basename(output), plan: "0064-prepared-safety", kind: "reuse-summary", status: "complete" })}\n`,
);
console.log(JSON.stringify({ groups: groups.size, queriesCompared: checks }));
