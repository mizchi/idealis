import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { compare } from "../../evals/experiments/rtla-parser/compare.js";
import { normalize } from "../../evals/experiments/rtla-parser/corpus.js";
import { classifySany } from "../../evals/experiments/rtla-parser/sany.js";

test("rtla native/Wasm preserve the semantic AST of the fixed finite-fragment corpus", async () => {
  const result = await compare();
  assert.ok(result.moduleComparisons > 150);
  console.log(
    JSON.stringify({
      modules: result.moduleComparisons,
      configs: result.configComparisons,
      nativeWasm: result.nativeWasmComparisons,
    }),
  );
});
test("SANY lexical EOF errors remain syntax rejections and infrastructure failures remain errors", async () => {
  const output = await readFile(
    "evals/runs/0026-rtla-development/sany/missing-comment-end/sany.log",
    "utf8",
  );
  assert.equal(classifySany(output, 0), "syntax");
  assert.equal(
    classifySany("Parsing file X\nFatal errors: could not find module", 1),
    "infrastructure",
  );
  assert.equal(classifySany("", null), "infrastructure");
});
test("AST comparison keeps binding order, prime, operator, values and bullet nesting", () => {
  assert.notDeepEqual(
    normalize({ kind: "Prime", expression: { kind: "Identifier", name: "x" } }),
    normalize({ kind: "Identifier", name: "x" }),
  );
  assert.notDeepEqual(
    normalize({ kind: "Apply", operator: "+", args: [1, 2] }),
    normalize({ kind: "Apply", operator: "-", args: [1, 2] }),
  );
  assert.notDeepEqual(
    normalize({ kind: "And", operands: [[1, 2], 3] }),
    normalize({ kind: "And", operands: [1, [2, 3]] }),
  );
});
