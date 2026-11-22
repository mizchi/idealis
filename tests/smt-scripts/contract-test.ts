import assert from "node:assert/strict";
import { test } from "node:test";
import { init } from "z3-solver";
import { checks } from "../../evals/experiments/smt-scripts/checks.js";
import { evaluateScript } from "../../evals/experiments/smt-subset/clients.js";

test(
  "SMT scripts agree with Z3, independent snapshots and shared work budgets",
  checks,
);

const source = `(declare-const x Int)\n${Array.from({ length: 12 }, (_, index) => `(push 1)\n(assert (> x ${index}))\n(check-sat)\n(pop 1)\n`).join("")}`;
const expected = Array.from({ length: 12 }, () => "sat");
const clobberStack = (api: Awaited<ReturnType<typeof init>>) => {
  const config = api.Z3.mk_config();
  api.Z3.set_param_value(config, "dot_proof_file", "v".repeat(600));
  api.Z3.del_config(config);
};

test("SMT script text survives main-thread stack reuse while Z3 parses it", async () => {
  const api = await init();
  const context = new api.Context("script-buffer");
  try {
    for (let round = 0; round < 16; round++) {
      const pending = evaluateScript(api, context.ptr, source);
      clobberStack(api);
      assert.deepEqual((await pending).trim().split(/\s+/), expected);
    }
  } finally {
    context.interrupt();
  }
});
