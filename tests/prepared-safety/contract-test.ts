import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  bounds,
  cases,
} from "../../evals/experiments/collections-temporal/cases.js";
import type { SafetyProgram } from "../../packages/idealis/src/contracts.js";
import {
  checkSafety,
  compileTlaSource,
  createPortableWasm,
  PortableCoreError,
  prepareSafetyProgram,
} from "../../packages/idealis/src/portable.js";

const bytes = await readFile(
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
);

test("prepared checks and batch keep fresh searches across results, errors and budgets", async () => {
  const core = await createPortableWasm(bytes);
  for (const input of cases) {
    for (const irVersion of [1, 2] as const) {
      let program: SafetyProgram;
      try {
        program = compileTlaSource(core, { ...input, irVersion });
      } catch (error) {
        if (!(error instanceof PortableCoreError)) throw error;
        assert.equal(
          input.expected,
          "error",
          `${input.name}:v${irVersion}: ${error.message}`,
        );
        continue;
      }
      const prepared = prepareSafetyProgram(core, program);
      const jobs = [1, 10000, 0, 17, 10000].map((maxWork) => ({
        bounds: { ...(input.bounds ?? bounds), maxWork },
      }));
      const expected = jobs.map((job) => {
        try {
          return {
            kind: "safety",
            result: checkSafety(core, program, job.bounds),
          } as const;
        } catch (error) {
          if (!(error instanceof PortableCoreError)) throw error;
          return {
            kind: "error",
            code: error.code,
            message: error.message,
          } as const;
        }
      });
      assert.deepEqual(
        prepared.checkBatch(jobs),
        expected,
        `${input.name}:v${irVersion}`,
      );
      assert.deepEqual(prepared.checkBatch(jobs), expected);
      for (const [index, job] of jobs.entries()) {
        const wanted = expected[index];
        assert.ok(wanted);
        if (wanted.kind === "safety")
          assert.deepEqual(prepared.check(job.bounds), wanted.result);
        else
          assert.throws(() => prepared.check(job.bounds), {
            code: wanted.code,
            message: wanted.message,
          });
      }
      assert.throws(() => prepared.checkBatch([]), { code: "protocol" });
      const first = jobs[0];
      assert.ok(first);
      assert.throws(
        () => prepared.checkBatch(Array.from({ length: 257 }, () => first)),
        { code: "protocol" },
      );
      prepared.release();
      prepared.release();
      assert.throws(() => prepared.check(input.bounds ?? bounds), {
        code: "protocol",
      });
    }
  }
});

test("preparation owns a snapshot and finite handles remain a different kind", async () => {
  const core = await createPortableWasm(bytes);
  const source = {
    source:
      "---- MODULE Snapshot ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nSafe == x < 3\n====",
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  };
  const program = compileTlaSource(core, source);
  const bounds = { maxDepth: 10, maxStates: 100, maxWork: 10000 };
  const expected = checkSafety(core, program, bounds);
  const prepared = prepareSafetyProgram(core, program);
  Object.assign(program, { name: "changed", initial: [], next: [] });
  assert.deepEqual(prepared.check(bounds), expected);
  const loaded = core.request({
    version: 1,
    request: {
      kind: "prepare-safety",
      program: compileTlaSource(core, source),
    },
  });
  assert.equal(loaded.kind, "safety-prepared");
  if (loaded.kind !== "safety-prepared") throw new Error("missing handle");
  assert.throws(
    () =>
      core.request({
        version: 1,
        request: { kind: "release-model", handle: loaded.handle },
      }),
    { code: "protocol" },
  );
  assert.deepEqual(prepared.check(bounds), expected);
  prepared.release();
  assert.deepEqual(
    core.request({
      version: 1,
      request: { kind: "release-safety", handle: loaded.handle },
    }),
    { kind: "released" },
  );
});
