import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";

export const sampleSides = ["before", "after", "after", "before"] as const;
export type SampleSide = (typeof sampleSides)[number];

export type SampleRecord = {
  readonly index: number;
  readonly side: string;
  readonly exit: number;
};

export type Estimates = ReadonlyMap<string, number>;

export type Comparison = {
  readonly name: string;
  readonly before: number;
  readonly after: number;
  readonly change: number;
  readonly samples: readonly [number, number, number, number];
};

export type Incomplete = {
  readonly benchmark: string;
  readonly reason: string;
};

export type Report = {
  readonly comparisons: readonly Comparison[];
  readonly incomplete: readonly Incomplete[];
};

export const sampleDirectory = (index: number, side: string): string =>
  `${index}-${side}`;

export const readEstimates = async (root: string): Promise<Estimates> => {
  const found = new Map<string, number>();
  const visit = async (directory: string, relative: string): Promise<void> => {
    const entries = await readdir(directory, { withFileTypes: true });
    if (
      relative.endsWith("/new") &&
      entries.some((entry) => entry.name === "estimates.json")
    ) {
      const estimates = JSON.parse(
        await readFile(join(directory, "estimates.json"), "utf8"),
      ) as { mean: { point_estimate: number } };
      found.set(
        relative.slice(0, -"/new".length).replace(/^\//, ""),
        estimates.mean.point_estimate,
      );
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        await visit(join(directory, entry.name), `${relative}/${entry.name}`);
      }
    }
  };
  await visit(root, "");
  return found;
};

export const sampleProblem = (
  records: readonly SampleRecord[],
): string | undefined => {
  if (records.length !== sampleSides.length) {
    return `${records.length} samples recorded, ${sampleSides.length} required`;
  }
  for (const [index, side] of sampleSides.entries()) {
    const record = records[index];
    if (
      record === undefined ||
      record.index !== index ||
      record.side !== side
    ) {
      return `sample ${index} is ${record?.side ?? "missing"}, ${side} required`;
    }
    if (record.exit !== 0) {
      return `sample ${index} (${side}) exited ${record.exit}`;
    }
  }
  return undefined;
};

export const compareBenchmark = (
  records: readonly SampleRecord[],
  estimates: readonly Estimates[],
):
  | { readonly comparisons: readonly Comparison[] }
  | { readonly problem: string } => {
  const problem = sampleProblem(records);
  if (problem !== undefined) return { problem };
  const [first, ...rest] = estimates;
  if (estimates.length !== sampleSides.length || first === undefined) {
    return { problem: `${estimates.length} estimate sets, 4 required` };
  }
  if (first.size === 0) return { problem: "sample 0 has no estimates" };
  for (const [offset, other] of rest.entries()) {
    const missing = [...first.keys()].filter((name) => !other.has(name));
    const extra = [...other.keys()].filter((name) => !first.has(name));
    if (missing.length > 0 || extra.length > 0) {
      return {
        problem: `sample ${offset + 1} differs from sample 0 in function set (${missing.length} missing, ${extra.length} extra)`,
      };
    }
  }
  const comparisons = [...first.keys()].sort().map((name): Comparison => {
    const values = estimates.map((set) => set.get(name) ?? Number.NaN) as [
      number,
      number,
      number,
      number,
    ];
    const before = (values[0] + values[3]) / 2;
    const after = (values[1] + values[2]) / 2;
    return {
      name,
      before,
      after,
      change: (after / before - 1) * 100,
      samples: values,
    };
  });
  return { comparisons };
};

export const compareAll = (
  expected: readonly string[],
  recorded: ReadonlyMap<
    string,
    {
      readonly records: readonly SampleRecord[];
      readonly estimates: readonly Estimates[];
    }
  >,
): Report => {
  const comparisons: Comparison[] = [];
  const incomplete: Incomplete[] = [];
  for (const benchmark of expected) {
    const entry = recorded.get(benchmark);
    if (entry === undefined) {
      incomplete.push({ benchmark, reason: "no recorded samples" });
      continue;
    }
    const result = compareBenchmark(entry.records, entry.estimates);
    if ("problem" in result) {
      incomplete.push({ benchmark, reason: result.problem });
    } else {
      comparisons.push(
        ...result.comparisons.map((row) => ({
          ...row,
          name: `${benchmark}/${row.name}`,
        })),
      );
    }
  }
  const unexpected = [...recorded.keys()].filter(
    (name) => !expected.includes(name),
  );
  for (const benchmark of unexpected) {
    incomplete.push({ benchmark, reason: "recorded but not expected" });
  }
  return { comparisons, incomplete };
};
