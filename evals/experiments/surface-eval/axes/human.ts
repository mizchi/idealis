import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Axis, Candidate, Context } from "../types.js";
import { lines } from "./cost.js";

const instructions =
  "You are reading models written in a modelling language you may not have seen. Answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.\n";

/** Source without comment lines or blocks, so no expectation leaks. */
function strip(text: string, comments: Candidate["comments"]): string {
  const kept: string[] = [];
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
    if (comments.line !== undefined && line.startsWith(comments.line)) continue;
    const trailing =
      comments.line === undefined ? -1 : raw.indexOf(` ${comments.line} `);
    kept.push(trailing > 0 ? raw.slice(0, trailing).trimEnd() : raw);
  }
  return kept.join("\n").replace(/\n{3,}/g, "\n\n");
}

function promptOf(context: Context, candidate: Candidate): string {
  let out = instructions;
  for (const shared of candidate.shared) {
    out += `\n## Shared definitions (${shared})\n\n\`\`\`\n${strip(context.read(`${candidate.dir}/${shared}`), candidate.comments)}\n\`\`\`\n`;
  }
  for (const task of context.tasks.filter((t) =>
    context.plan.tasks.includes(t.id),
  )) {
    const file = candidate.files[task.id];
    if (file === undefined) continue;
    out += `\n## Model ${task.id}\n\n\`\`\`\n${strip(context.read(`${candidate.dir}/${file}`), candidate.comments)}\n\`\`\`\n\n`;
    for (const q of task.questions) {
      out += `- \`${task.id}.${q.id}\`: ${q.text} Options: ${q.options.map((o) => `"${o}"`).join(", ")}\n`;
    }
  }
  return out;
}

export const human: Axis = {
  id: "human",
  title: "人間への理解容易性（代理）",
  question: "構文の説明なしで、読み手が振る舞いを読み取れるか",
  kind: "agent",
  scoring: "新しい読み手の、振る舞いの問いの正答率（本物の人間ではない）",
  prepare(context) {
    const dir = join(context.run, "human");
    mkdirSync(join(dir, "answers"), { recursive: true });
    for (const candidate of context.candidates) {
      writeFileSync(
        join(dir, `${candidate.id}.md`),
        promptOf(context, candidate),
      );
    }
  },
  measure(context) {
    const metrics: Record<string, Record<string, number | string | null>> = {};
    const score: Record<string, number | null> = {};
    const notes: string[] = [
      "readers are fresh subagents given no description of the syntax; comments are stripped",
    ];
    const questions = context.tasks
      .filter((t) => context.plan.tasks.includes(t.id))
      .flatMap((t) =>
        t.questions.map((q) => ({ id: `${t.id}.${q.id}`, answer: q.answer })),
      );
    for (const candidate of context.candidates) {
      const answers: Record<string, string>[] = [];
      for (let reader = 0; reader < context.plan.human.readers; reader += 1) {
        const file = join(
          context.run,
          "human",
          "answers",
          `${candidate.id}-${reader}.json`,
        );
        if (existsSync(file))
          answers.push(
            JSON.parse(readFileSync(file, "utf8")) as Record<string, string>,
          );
      }
      if (answers.length < context.plan.human.readers) {
        metrics[candidate.id] = {
          correct: null,
          questions: questions.length,
          readers: answers.length,
        };
        score[candidate.id] = null;
        notes.push(
          `${candidate.id}: ${answers.length} of ${context.plan.human.readers} readers collected`,
        );
        continue;
      }
      let correct = 0;
      const wrong: string[] = [];
      for (const given of answers) {
        for (const q of questions) {
          if (given[q.id] === q.answer) correct += 1;
          else wrong.push(q.id);
        }
      }
      const total = questions.length * answers.length;
      const prompt = readFileSync(
        join(context.run, "human", `${candidate.id}.md`),
        "utf8",
      );
      metrics[candidate.id] = {
        correct,
        questions: total,
        promptLines: lines(prompt, {}),
        // Expected verdicts left in the code itself (not comments): hints
        // the questions do not ask for directly, recorded as plan 0109 says.
        leaks: (
          prompt.match(
            /\b(holds|violated|unreachable|ReachableAt|expect\w*)\b/gi,
          ) ?? []
        ).length,
        wrong: wrong.join(" "),
      };
      score[candidate.id] = correct / total;
    }
    return { axis: "human", metrics, score, notes };
  },
};
