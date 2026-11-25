import { mean, type Axis } from "../types.js";

/** Evidence keys of the implementation axis (plan 0110). */
const parts: [string, string][] = [
  [
    "codegen",
    "generates code for implementation languages (2+ 1, one 0.5, none 0)",
  ],
  [
    "conformance",
    "checks an implementation against the model (examples or traces 1, partly 0.5, none 0)",
  ],
  [
    "enforcement",
    "enforces invariants on the implementation side (generated code or types 1, run-time checks only 0.5, none 0)",
  ],
];

export const impl: Axis = {
  id: "impl",
  title: "実装への接続",
  question: "モデルから実装の言語へ繋がるか",
  kind: "rubric",
  scoring: "生成・照合・不変条件の強制の3つ（rubric）の平均",
  measure(context) {
    const metrics: Record<string, Record<string, number | null>> = {};
    const score: Record<string, number | null> = {};
    for (const candidate of context.candidates) {
      const values = parts.map(([key]) => candidate.evidence[key] ?? null);
      metrics[candidate.id] = Object.fromEntries(
        parts.map(([key], i) => [key, values[i] ?? null]),
      );
      score[candidate.id] = mean(values);
    }
    return {
      axis: "impl",
      metrics,
      score,
      notes: parts.map(([key, text]) => `${key}: ${text}`),
    };
  },
};
