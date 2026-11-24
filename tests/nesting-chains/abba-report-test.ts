import assert from "node:assert/strict";
import { test } from "node:test";
import {
  compareAll,
  sampleSides,
  type Estimates,
  type SampleRecord,
} from "../../evals/experiments/nesting-chains/abba/samples.js";

const records = (exits: readonly number[]): SampleRecord[] =>
  exits.map((exit, index) => ({
    index,
    side: sampleSides[index] ?? "?",
    exit,
  }));
const estimates = (values: Record<string, number>): Estimates =>
  new Map(Object.entries(values));
const complete = (name: string) => ({
  records: records([0, 0, 0, 0]),
  estimates: [
    estimates({ [name]: 100 }),
    estimates({ [name]: 90 }),
    estimates({ [name]: 110 }),
    estimates({ [name]: 100 }),
  ],
});

test("a benchmark with four ordered successful samples yields rows", () => {
  const report = compareAll(["a"], new Map([["a", complete("f")]]));
  assert.deepEqual(report.incomplete, []);
  assert.equal(report.comparisons[0]?.name, "a/f");
  assert.equal(report.comparisons[0]?.change, 0);
});

test("a failed first sample lists the benchmark as incomplete", () => {
  const failedFirst = {
    records: records([-6]),
    estimates: [new Map(), new Map(), new Map(), new Map()],
  };
  const report = compareAll(
    ["a", "b"],
    new Map([
      ["a", failedFirst],
      ["b", complete("g")],
    ]),
  );
  assert.deepEqual(report.incomplete, [
    { benchmark: "a", reason: "1 samples recorded, 4 required" },
  ]);
  assert.deepEqual(
    report.comparisons.map((row) => row.name),
    ["b/g"],
  );
});

test("a failed later sample, a wrong order and a missing run are incomplete", () => {
  const later = { ...complete("f"), records: records([0, 0, 3, 0]) };
  const wrongOrder = {
    ...complete("f"),
    records: [0, 1, 2, 3].map((index) => ({ index, side: "before", exit: 0 })),
  };
  const report = compareAll(
    ["a", "b", "c"],
    new Map([
      ["a", later],
      ["b", wrongOrder],
    ]),
  );
  assert.deepEqual(
    report.incomplete.map((item) => [item.benchmark, item.reason]),
    [
      ["a", "sample 2 (after) exited 3"],
      ["b", "sample 1 is before, after required"],
      ["c", "no recorded samples"],
    ],
  );
  assert.equal(report.comparisons.length, 0);
});

test("estimates that are empty or differ in function set are incomplete", () => {
  const empty = {
    ...complete("f"),
    estimates: [new Map(), ...complete("f").estimates.slice(1)],
  };
  const differing = {
    ...complete("f"),
    estimates: [
      estimates({ f: 1 }),
      estimates({ f: 1 }),
      estimates({ f: 1, h: 1 }),
      estimates({ f: 1 }),
    ],
  };
  const report = compareAll(
    ["a", "b"],
    new Map([
      ["a", empty],
      ["b", differing],
    ]),
  );
  assert.deepEqual(
    report.incomplete.map((item) => item.reason),
    [
      "sample 0 has no estimates",
      "sample 2 differs from sample 0 in function set (0 missing, 1 extra)",
    ],
  );
});

test("a recorded benchmark that was not expected is reported", () => {
  const report = compareAll(
    ["a"],
    new Map([
      ["a", complete("f")],
      ["z", complete("f")],
    ]),
  );
  assert.deepEqual(report.incomplete, [
    { benchmark: "z", reason: "recorded but not expected" },
  ]);
});

test("a missing, nonnumeric or nonfinite estimate makes the benchmark incomplete", () => {
  for (const value of [undefined, "1", Number.NaN, Number.POSITIVE_INFINITY]) {
    const bad = {
      ...complete("f"),
      estimates: [
        estimates({ f: 100 }),
        new Map([["f", value as unknown as number]]),
        estimates({ f: 110 }),
        estimates({ f: 100 }),
      ],
    };
    const report = compareAll(["a"], new Map([["a", bad]]));
    assert.deepEqual(report.comparisons, []);
    assert.deepEqual(report.incomplete, [
      {
        benchmark: "a",
        reason: "sample 1 has a missing or nonfinite estimate for f",
      },
    ]);
  }
});

test("a nonpositive baseline makes the benchmark incomplete", () => {
  for (const [slot, offset] of [
    [0, 0],
    [3, 3],
  ] as const) {
    const sets: Estimates[] = [...complete("f").estimates];
    sets[slot] = estimates({ f: 0 });
    const report = compareAll(
      ["a"],
      new Map([["a", { ...complete("f"), estimates: sets }]]),
    );
    assert.deepEqual(report.comparisons, []);
    assert.deepEqual(report.incomplete, [
      {
        benchmark: "a",
        reason: `sample ${offset} has a nonpositive baseline for f`,
      },
    ]);
  }
});
