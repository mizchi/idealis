import { mean, type Axis } from "../types.js";

export const machine: Axis = {
  id: "machine",
  title: "機械での理解容易性",
  question: "道具がモデルを解析し、検査できるか",
  kind: "mixed",
  scoring:
    "解析できる木（rubric: 1・0.5・0）と、toolchainの検査が今通るか（1・0、使えなければ未測定）の平均",
  measure(context) {
    const outcomes = context.checks();
    const metrics: Record<string, Record<string, number | string | null>> = {};
    const score: Record<string, number | null> = {};
    for (const candidate of context.candidates) {
      const outcome = outcomes.find((o) => o.candidate === candidate.id);
      const passed =
        outcome === undefined || outcome.status === "unavailable"
          ? null
          : outcome.status === "passed"
            ? 1
            : 0;
      metrics[candidate.id] = {
        tree: candidate.tree,
        toolchain: outcome?.status ?? null,
      };
      score[candidate.id] = mean([candidate.tree, passed]);
    }
    return {
      axis: "machine",
      metrics,
      score,
      notes: [
        "tree: rubric (1 an analysable tree, 0.5 analysable only inside the host, 0 host functions)",
        "toolchain: the candidate's check commands, run in this run",
      ],
    };
  },
};
