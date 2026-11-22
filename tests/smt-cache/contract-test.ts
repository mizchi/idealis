import { test } from "node:test";
import { checks } from "../../evals/experiments/smt-cache/checks.js";

test(
  "cached SMT checks match plain runs on native and Wasm and stay opt-in",
  checks,
);
