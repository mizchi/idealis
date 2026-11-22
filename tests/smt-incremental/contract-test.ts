import { test } from "node:test";
import { checks } from "../../evals/experiments/smt-incremental/checks.js";

test(
  "incremental SMT contexts and the check cache agree with Z3 on native and Wasm",
  checks,
);
