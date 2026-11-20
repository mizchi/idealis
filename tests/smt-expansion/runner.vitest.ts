import { test } from "vite-plus/test";
import { checks } from "../../evals/experiments/smt-expansion/checks.js";

test(
  "SMT expansion agrees with Z3 and preserves independent assertion scopes",
  checks,
);
