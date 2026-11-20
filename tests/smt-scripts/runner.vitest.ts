import { test } from "vite-plus/test";
import { checks } from "../../evals/experiments/smt-scripts/checks.js";

test(
  "SMT scripts agree with Z3, independent snapshots and shared work budgets",
  checks,
);
