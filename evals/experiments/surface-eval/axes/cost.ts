import type { Axis, Candidate, Context } from "../types.js";

/** Lines that are not blank or comments. */
export function lines(text: string, comments: Candidate["comments"]): number {
  let count = 0;
  let inBlock = false;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (comments.block !== undefined) {
      const [open, close] = comments.block;
      if (inBlock) {
        if (line.includes(close)) inBlock = false;
        continue;
      }
      if (line.startsWith(open)) {
        if (!line.slice(open.length).includes(close)) inBlock = true;
        continue;
      }
    }
    if (line === "") continue;
    if (comments.line !== undefined && line.startsWith(comments.line)) continue;
    count += 1;
  }
  return count;
}

export function modelLines(context: Context, candidate: Candidate): number {
  return context.plan.tasks.reduce((total, task) => {
    const file = candidate.files[task];
    return file === undefined
      ? total
      : total +
          lines(context.read(`${candidate.dir}/${file}`), candidate.comments);
  }, 0);
}

export const cost: Axis = {
  id: "cost",
  title: "道具の費用",
  question: "書く量と検査の時間はどれだけか",
  kind: "automatic",
  scoring: "候補の中で最小の行数を1とした比（最小/自分）",
  measure(context) {
    const outcomes = context.checks();
    const metrics: Record<string, Record<string, number | string | null>> = {};
    const counts = new Map(
      context.candidates.map((c) => [c.id, modelLines(context, c)]),
    );
    const smallest = Math.min(...counts.values());
    const score: Record<string, number | null> = {};
    for (const candidate of context.candidates) {
      const shared = candidate.shared.reduce(
        (total, file) =>
          total +
          lines(context.read(`${candidate.dir}/${file}`), candidate.comments),
        0,
      );
      const outcome = outcomes.find((o) => o.candidate === candidate.id);
      const count = counts.get(candidate.id) ?? 0;
      metrics[candidate.id] = {
        modelLines: count,
        sharedLines: shared,
        checkSeconds:
          outcome === undefined || outcome.status === "unavailable"
            ? null
            : Number(outcome.seconds.toFixed(1)),
      };
      score[candidate.id] = count === 0 ? null : smallest / count;
    }
    return {
      axis: "cost",
      metrics,
      score,
      notes: [
        "lines exclude blank and comment lines; shared DSL or schema lines are reported, not scored",
      ],
    };
  },
};
