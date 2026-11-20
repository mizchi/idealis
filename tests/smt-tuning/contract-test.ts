import { test } from "node:test";
import { checks } from "../../evals/experiments/smt-tuning/checks.js";

test(
  "benchmark fixtures and immutable query snapshots retain Z3 semantics",
  checks,
);
