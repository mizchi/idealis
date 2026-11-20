import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  createZ3,
  wasmPath,
} from "../../evals/experiments/smt-subset/clients.js";
import {
  generated,
  handCases,
} from "../../evals/experiments/smt-subset/corpus.js";
import { verify } from "../../evals/experiments/smt-subset/verify.js";
import { createWasm } from "../../evals/experiments/smt-subset/wasm.js";

test("native/Wasm subset, Z3 and known fault reproduction agree", async () => {
  await verify(
    [...handCases(), ...generated(0x30a1, 8)],
    await createWasm(await readFile(wasmPath)),
    () => {},
  );
});

test("scoped Z3 queries restore assertions after pop", async () => {
  const z3 = await createZ3();
  const scoped = z3.scoped({
    name: "initial",
    family: "reuse",
    integers: ["x"],
    booleans: [],
    source: "(set-logic QF_IDL)(declare-const x Int)(assert (= x 1))",
    expected: "sat",
    iterations: 1,
  });
  try {
    for (const value of [1, 2, 1]) {
      const input = {
        name: `scope-${value}`,
        family: "reuse",
        integers: ["x"],
        booleans: [],
        source: `(set-logic QF_IDL)(declare-const x Int)(assert (= x ${value}))`,
        expected: "sat" as const,
        iterations: 1,
      };
      const result = await scoped.check(input);
      assert.equal(result.status, "sat");
      assert.equal(result.values.x, String(value));
      await z3.validate(input, result);
    }
  } finally {
    scoped.close();
    await z3.close();
  }
});
