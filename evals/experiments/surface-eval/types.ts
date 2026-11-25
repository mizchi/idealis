// The surface-language evaluation loop (plan 0109): candidates, tasks,
// plans and the contract every axis implements.

export interface Check {
  cwd: string;
  run: string[];
  each: boolean;
  env?: string[];
}

export interface Candidate {
  id: string;
  dir: string;
  files: Record<string, string>;
  shared: string[];
  comments: { line?: string; block?: [string, string] };
  check: Check[];
  checkInputs?: string[];
  adapter: string | null;
  tree: number;
  verification: string[];
  evidence: Record<string, number>;
}

export interface Question {
  id: string;
  text: string;
  options: string[];
  answer: string;
}

export interface Task {
  id: string;
  questions: Question[];
}

export interface Plan {
  version: number;
  id: string;
  candidates: string[];
  tasks: string[];
  axes: string[];
  /** The tasks and human-axis questions (default `tasks.json`). */
  tasksFile?: string;
  human: { readers: number; model: string; batch: string };
  weights: Record<string, number> | null;
}

/** The outcome of running one candidate's toolchain checks. */
export interface CheckOutcome {
  candidate: string;
  status: "passed" | "failed" | "unavailable";
  seconds: number;
  detail: string;
}

export interface Context {
  root: string;
  run: string;
  plan: Plan;
  candidates: Candidate[];
  tasks: Task[];
  /** Runs the candidates' checks once per run and caches the outcome. */
  checks(): CheckOutcome[];
  /** Reads a file of the repository. */
  read(path: string): string;
}

/**
 * One evaluation axis. `automatic` axes measure with tools, `rubric` axes
 * count recorded evidence, `agent` axes ask fresh readers whose answers
 * are collected into the run between `prepare` and `score`.
 */
export interface Axis {
  id: string;
  title: string;
  question: string;
  kind: "automatic" | "rubric" | "agent" | "mixed";
  /** How the score in [0, 1] is computed from the metrics. */
  scoring: string;
  /** Writes what an agent axis needs collected; automatic axes may omit. */
  prepare?(context: Context): void;
  measure(context: Context): AxisResult;
}

export interface AxisResult {
  axis: string;
  /** candidate -> metric -> value; null when not measured. */
  metrics: Record<string, Record<string, number | string | null>>;
  /** candidate -> score in [0, 1], or null when not measured. */
  score: Record<string, number | null>;
  notes: string[];
}

export function mean(values: (number | null)[]): number | null {
  const known = values.filter((v): v is number => v !== null);
  if (known.length !== values.length || known.length === 0) return null;
  return known.reduce((a, b) => a + b, 0) / known.length;
}
