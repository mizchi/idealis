import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { mean, type Axis, type Context } from "../types.js";

/** The checks compared through the IR (as crates/rcheck/tests/adapters.rs). */
const observed: Record<string, string[]> = {
  permissions: ["NoLeak", "OnlyGranted", "SecretRead", "UnclearedSecret"],
  bank: ["NonNegative", "Rich"],
  order: ["FraudCancelled"],
  sessions: [
    "Capacity",
    "LockedByAdmin",
    "LockedIsActive",
    "Full",
    "ThreeActive",
  ],
  service: ["Disjoint"],
};

const kinds = ["complete-search", "bounded", "inductive", "proof"];

const rcheck = (context: Context) =>
  join(context.root, "crates/rcheck/target/release/rcheck");

/** Reference verdicts of a model or IR file, by check name. */
function verdicts(
  context: Context,
  file: string,
): Map<string, string> | string {
  try {
    const out = execFileSync(
      rcheck(context),
      [file, "--checker", "reference"],
      {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    const reports = JSON.parse(out) as {
      question: string;
      answers: { verdict: string; depth?: number }[];
    }[];
    return new Map(
      reports.map((r) => {
        const answer = r.answers[0];
        return [
          r.question,
          answer === undefined
            ? "none"
            : `${answer.verdict}${answer.depth === undefined ? "" : `@${answer.depth}`}`,
        ];
      }),
    );
  } catch (error) {
    return String((error as { stderr?: string }).stderr ?? error).trim();
  }
}

export const formal: Axis = {
  id: "formal",
  title: "形式化との相性",
  question: "核のIRと検査器にどこまで載るか",
  kind: "mixed",
  scoring:
    "IRに写せて参照評価器の判定がrmodelと一致した題材の割合（adapterがなければ0）と、答えを出せた検査の種類の割合（4種類）の平均",
  measure(context) {
    const metrics: Record<string, Record<string, number | string | null>> = {};
    const score: Record<string, number | null> = {};
    const notes: string[] = [];
    const ir = join(context.run, "formal", "ir");
    mkdirSync(ir, { recursive: true });
    const rmodel = context.candidates.find((c) => c.id === "rmodel");
    for (const candidate of context.candidates) {
      let matched = 0;
      let unsupported = 0;
      for (const task of context.plan.tasks) {
        if (candidate.adapter === null || rmodel === undefined) continue;
        const reference = verdicts(
          context,
          join(context.root, rmodel.dir, rmodel.files[task] ?? ""),
        );
        // rmodel's own IR is written now, through `rcheck generate <model> ir`.
        let file = join(context.root, candidate.adapter, `${task}.json`);
        if (candidate.id === "rmodel") {
          file = join(ir, `rmodel-${task}.json`);
          writeFileSync(
            file,
            execFileSync(
              rcheck(context),
              [
                "generate",
                join(context.root, candidate.dir, candidate.files[task] ?? ""),
                "ir",
              ],
              {
                encoding: "utf8",
              },
            ),
          );
        }
        if (!existsSync(file)) {
          notes.push(`${candidate.id}/${task}: no adapter output`);
          continue;
        }
        if (
          "unsupported" in (JSON.parse(readFileSync(file, "utf8")) as object)
        ) {
          unsupported += 1;
          continue;
        }
        const actual = verdicts(context, file);
        const names = observed[task] ?? [];
        const agree =
          typeof reference !== "string" &&
          typeof actual !== "string" &&
          names.every(
            (name) =>
              actual.get(name) !== undefined &&
              actual.get(name) === reference.get(name),
          );
        if (agree) matched += 1;
        else
          notes.push(
            `${candidate.id}/${task}: ${typeof actual === "string" ? actual : "verdicts differ or a check is omitted"}`,
          );
      }
      const tasks = context.plan.tasks.length;
      const reach = candidate.adapter === null ? 0 : matched / tasks;
      const breadth =
        kinds.filter((k) => candidate.verification.includes(k)).length /
        kinds.length;
      metrics[candidate.id] = {
        adapter: candidate.adapter === null ? 0 : 1,
        irMatched: candidate.adapter === null ? null : matched,
        irUnsupported: candidate.adapter === null ? null : unsupported,
        verification: candidate.verification.join(" "),
        liveness: candidate.evidence["liveness"] ?? null,
      };
      score[candidate.id] = mean([reach, breadth]);
    }
    notes.push(
      "verification kinds: rubric from the candidate's checks (complete-search, bounded, inductive, proof)",
    );
    return { axis: "formal", metrics, score, notes };
  },
};
