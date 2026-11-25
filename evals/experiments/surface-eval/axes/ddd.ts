import { mean, type Axis } from "../types.js";

/** DDD concepts and the 0107 evidence keys that show each. */
const concepts: [string, string][] = [
  ["entity identity with a check-time size", "identity"],
  ["configuration and its premises", "config"],
  ["value objects (integers and data)", "ints"],
  ["algebraic data with payloads and total match", "adt"],
  ["impossible states ruled out by types", "typeExclusion"],
  ["business rules over transitions", "step"],
  ["optional associations", "lone"],
];

export const ddd: Axis = {
  id: "ddd",
  title: "DDD記述",
  question: "DDDの概念をそのまま書けるか",
  kind: "rubric",
  scoring: "DDDの概念7つ（直接1・回避0.5・書けない0）の平均",
  measure(context) {
    const metrics: Record<string, Record<string, number | null>> = {};
    const score: Record<string, number | null> = {};
    for (const candidate of context.candidates) {
      const values = concepts.map(([, key]) => candidate.evidence[key] ?? null);
      metrics[candidate.id] = Object.fromEntries(
        concepts.map(([, key], i) => [key, values[i] ?? null]),
      );
      score[candidate.id] = mean(values);
    }
    return {
      axis: "ddd",
      metrics,
      score,
      notes: [
        "rubric: evidence from each candidate's model and writer report",
        ...concepts.map(([text, key]) => `${key}: ${text}`),
      ],
    };
  },
};
