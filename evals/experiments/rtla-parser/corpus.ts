import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { cases } from "../collections-temporal/cases.js";
import { boundaries } from "../tlc-differential/corpus.js";

export interface Input {
  readonly name: string;
  readonly source: string;
  readonly config: string;
}
export const archived = [
  "evals/runs/0015-development-final/inputs.json",
  "evals/runs/0015-confirmation-isolated/inputs.json",
  "evals/runs/0019-single-model/cases.json",
  "evals/runs/0022-readme-comparison/cases.json",
];
export const corpus = async (): Promise<readonly Input[]> => {
  const inputs: Input[] = [...cases, ...boundaries()];
  for (const path of archived) {
    const saved: unknown = JSON.parse(await readFile(path, "utf8"));
    assert.ok(Array.isArray(saved));
    for (const input of saved) {
      assert.ok(
        isRecord(input) &&
          typeof input.name === "string" &&
          typeof input.source === "string" &&
          typeof input.config === "string",
      );
      inputs.push({
        name: `${path}:${input.name}`,
        source: input.source,
        config: input.config,
      });
    }
  }
  const seen = new Set<string>();
  return inputs.filter((input) => {
    const key = JSON.stringify([input.source, input.config]);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
};

const metadata = new Set([
  "location",
  "range",
  "ranges",
  "extendsRanges",
  "bodyRange",
  "head",
  "paramRanges",
  "nameRanges",
  "otherRange",
  "argumentsRange",
  "selectorRanges",
  "argumentRanges",
  "prefixEnds",
  "spelling",
  "rawConstants",
]);
export const normalize = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(normalize);
  if (!isRecord(value)) return value;
  const result = Object.fromEntries(
    Object.entries(value)
      .filter(
        ([key, inner]) =>
          inner !== undefined &&
          !metadata.has(key) &&
          !(key === "text" && value.kind === "Number"),
      )
      .map(([key, inner]) => [key, normalize(inner)]),
  );
  if (value.kind === "Lambda" && Array.isArray(value.params))
    result.params = value.params.map((param) =>
      typeof param === "string" ? { name: param, arity: 0 } : normalize(param),
    );
  return result;
};
export const moduleFields = (value: unknown): unknown => {
  assert.ok(isRecord(value));
  for (const field of ["modules", "instances", "uses"])
    if (field in value) assert.deepEqual(value[field], [], field);
  return normalize(
    Object.fromEntries(
      [
        "name",
        "extends",
        "constants",
        "variables",
        "definitions",
        "assumptions",
        "theorems",
      ].map((field) => [field, value[field]]),
    ),
  );
};
