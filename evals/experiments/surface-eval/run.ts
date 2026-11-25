// The surface-language evaluation loop (plan 0109).
//
//   node dist/evals/experiments/surface-eval/run.js prepare <plan.json> <run-dir>
//   node dist/evals/experiments/surface-eval/run.js score <plan.json> <run-dir>
//
// prepare refuses an existing run, records a manifest, runs the toolchain
// checks once, measures the automatic and rubric axes and writes what the
// agent axes need collected (human: one prompt per candidate). After the
// answers are saved under the run, score measures the agent axes and
// writes scorecard.json and scorecard.md from the saved results; it can be
// run again and gives the same scorecard.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { isAbsolute, join } from "node:path";
import { axes } from "./axes/index.js";
import type {
  AxisResult,
  Candidate,
  CheckOutcome,
  Context,
  Plan,
  Task,
} from "./types.js";

const root = process.cwd();
const here = "evals/experiments/surface-eval";
const read = (path: string) =>
  readFileSync(isAbsolute(path) ? path : join(root, path), "utf8");
const json = <T>(path: string) => JSON.parse(read(path)) as T;

function context(planPath: string, run: string): Context {
  const plan = json<Plan>(planPath);
  const candidates = json<{ candidates: Candidate[] }>(
    `${here}/candidates.json`,
  ).candidates.filter((c) => plan.candidates.includes(c.id));
  const tasks = json<{ tasks: Task[] }>(
    `${here}/${plan.tasksFile ?? "tasks.json"}`,
  ).tasks.filter((t) => plan.tasks.includes(t.id));
  for (const id of plan.axes)
    if (axes[id] === undefined) throw new Error(`unknown axis ${id}`);
  for (const id of plan.candidates)
    if (!candidates.some((c) => c.id === id))
      throw new Error(`unknown candidate ${id}`);
  let cached: CheckOutcome[] | undefined;
  const context: Context = {
    root,
    run,
    plan,
    candidates,
    tasks,
    read,
    checks() {
      const file = join(run, "checks.json");
      if (cached !== undefined) return cached;
      if (existsSync(file))
        return (cached = JSON.parse(
          readFileSync(file, "utf8"),
        ) as CheckOutcome[]);
      cached = candidates.map((c) => runChecks(c, plan));
      writeFileSync(file, `${JSON.stringify(cached, null, 2)}\n`);
      return cached;
    },
  };
  return context;
}

/** Runs a candidate's check commands; a missing tool is unavailable, never passed. */
function runChecks(candidate: Candidate, plan: Plan): CheckOutcome {
  const started = Date.now();
  for (const check of candidate.check) {
    const missing = (check.env ?? []).concat(
      [...check.run, check.cwd].flatMap((part) =>
        [...part.matchAll(/\{env:([A-Z0-9_]+)\}/g)].map((m) => m[1] ?? ""),
      ),
    );
    const absent = missing.filter((name) => !process.env[name]);
    if (absent.length > 0) {
      return {
        candidate: candidate.id,
        status: "unavailable",
        seconds: 0,
        detail: `unset: ${absent.join(", ")}`,
      };
    }
    const files = check.each
      ? plan.tasks
          .map((t) => candidate.files[t])
          .filter((f): f is string => f !== undefined)
      : [""];
    for (const file of files) {
      const fill = (part: string) =>
        part
          .replaceAll("{dir}", candidate.dir)
          .replaceAll("{file}", file)
          .replaceAll("{abs}", join(root, candidate.dir, file))
          .replace(
            /\{env:([A-Z0-9_]+)\}/g,
            (_, name: string) => process.env[name] ?? "",
          );
      const cwd = fill(check.cwd);
      const dir =
        cwd === "{dir}" || cwd === candidate.dir
          ? join(root, candidate.dir)
          : isAbsolute(cwd)
            ? cwd
            : join(root, cwd);
      const [command, ...args] = check.run.map(fill);
      try {
        execFileSync(command ?? "", args, {
          cwd: dir,
          stdio: ["ignore", "pipe", "pipe"],
          timeout: 900_000,
        });
      } catch (error) {
        const detail = String((error as { stderr?: Buffer }).stderr ?? error)
          .trim()
          .split("\n")
          .slice(-3)
          .join(" | ");
        return {
          candidate: candidate.id,
          status: "failed",
          seconds: (Date.now() - started) / 1000,
          detail: `${file}: ${detail}`,
        };
      }
    }
  }
  return {
    candidate: candidate.id,
    status: "passed",
    seconds: (Date.now() - started) / 1000,
    detail: "",
  };
}

function sha256(path: string): string {
  return createHash("sha256").update(read(path)).digest("hex");
}

function prepare(planPath: string, run: string): void {
  if (existsSync(join(run, "manifest.json")))
    throw new Error(`${run} exists: runs are never overwritten`);
  const ctx = context(planPath, run);
  const inputs = [
    planPath,
    `${here}/candidates.json`,
    `${here}/${ctx.plan.tasksFile ?? "tasks.json"}`,
    ...ctx.candidates.flatMap((candidate) =>
      [
        ...Object.values(candidate.files),
        ...candidate.shared,
        ...(candidate.checkInputs ?? []),
      ].map((file) => join(candidate.dir, file)),
    ),
  ];
  const inputHashes = Object.fromEntries(
    inputs.map((path) => [path, sha256(path)]),
  );
  const git = (...args: string[]) =>
    execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
  mkdirSync(join(run, "results"), { recursive: true });
  writeFileSync(
    join(run, "manifest.json"),
    `${JSON.stringify(
      {
        plan: planPath,
        revision: git("rev-parse", "HEAD"),
        dirty: git("status", "--porcelain").split("\n").filter(Boolean),
        inputs: inputHashes,
        node: process.version,
        started: new Date().toISOString(),
      },
      null,
      2,
    )}\n`,
  );
  ctx.checks();
  for (const id of ctx.plan.axes) {
    const axis = axes[id];
    if (axis === undefined) continue;
    axis.prepare?.(ctx);
    if (axis.kind !== "agent") writeResult(run, axis.measure(ctx));
  }
}

function writeResult(run: string, result: AxisResult): void {
  writeFileSync(
    join(run, "results", `${result.axis}.json`),
    `${JSON.stringify(result, null, 2)}\n`,
  );
}

function format(value: number | string | null): string {
  if (value === null) return "—";
  if (typeof value === "number")
    return Number.isInteger(value) ? String(value) : value.toFixed(2);
  return value === "" ? "" : value;
}

function score(planPath: string, run: string): void {
  if (!existsSync(join(run, "manifest.json")))
    throw new Error(`${run}: run prepare first`);
  const ctx = context(planPath, run);
  const results: AxisResult[] = [];
  for (const id of ctx.plan.axes) {
    const axis = axes[id];
    if (axis === undefined) continue;
    if (axis.kind === "agent") writeResult(run, axis.measure(ctx));
    results.push(
      JSON.parse(
        readFileSync(join(run, "results", `${id}.json`), "utf8"),
      ) as AxisResult,
    );
  }
  const weights = ctx.plan.weights;
  const total = (candidate: string): number | null => {
    if (weights === null) return null;
    let sum = 0;
    let weight = 0;
    for (const r of results) {
      const s = r.score[candidate];
      const w = weights[r.axis] ?? 0;
      if (s === null || s === undefined) return null;
      sum += s * w;
      weight += w;
    }
    return weight === 0 ? null : sum / weight;
  };
  const card = {
    plan: ctx.plan.id,
    axes: results.map((r) => {
      const axis = axes[r.axis];
      return {
        id: r.axis,
        title: axis?.title,
        question: axis?.question,
        kind: axis?.kind,
        scoring: axis?.scoring,
        score: r.score,
      };
    }),
    total:
      weights === null
        ? null
        : Object.fromEntries(ctx.candidates.map((c) => [c.id, total(c.id)])),
  };
  writeFileSync(
    join(run, "scorecard.json"),
    `${JSON.stringify(card, null, 2)}\n`,
  );
  let md = `# 成績表（${ctx.plan.id}）\n\n各軸は0〜1。—は未測定（合格に数えない）。1つの点にまとめない${weights === null ? "（重みなし）" : "（重みはplanのもの）"}。\n\n`;
  md += `| 候補 | ${results.map((r) => axes[r.axis]?.title ?? r.axis).join(" | ")}${weights === null ? "" : " | 重み付き"} |\n`;
  md += `| --- | ${results.map(() => "---:").join(" | ")}${weights === null ? "" : " | ---:"} |\n`;
  for (const c of ctx.candidates) {
    md += `| ${c.id} | ${results.map((r) => format(r.score[c.id] ?? null)).join(" | ")}${weights === null ? "" : ` | ${format(total(c.id))}`} |\n`;
  }
  for (const r of results) {
    const axis = axes[r.axis];
    md += `\n## ${axis?.title ?? r.axis}（${r.axis}、${axis?.kind}）\n\n${axis?.question}。得点: ${axis?.scoring}。\n\n`;
    const keys = [
      ...new Set(Object.values(r.metrics).flatMap((m) => Object.keys(m))),
    ];
    md += `| 候補 | ${keys.join(" | ")} |\n| --- | ${keys.map(() => "---").join(" | ")} |\n`;
    for (const c of ctx.candidates) {
      md += `| ${c.id} | ${keys.map((k) => format(r.metrics[c.id]?.[k] ?? null)).join(" | ")} |\n`;
    }
    if (r.notes.length > 0)
      md += `\n${r.notes.map((n) => `- ${n}`).join("\n")}\n`;
  }
  writeFileSync(join(run, "scorecard.md"), md);
  process.stdout.write(md);
}

const [command, planPath, run] = process.argv.slice(2);
if (planPath === undefined || run === undefined) {
  throw new Error("usage: run.js prepare|score <plan.json> <run-dir>");
}
if (command === "prepare") prepare(planPath, run);
else if (command === "score") score(planPath, run);
else throw new Error(`unknown command ${command}`);
