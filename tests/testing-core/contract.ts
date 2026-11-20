import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { verify } from "../../evals/experiments/testing-core/verify.js";
import type { TestingPlan } from "../../packages/idealis/src/contracts.js";
import {
  createTestingWasm,
  TestingError,
} from "../../packages/idealis/src/testing.js";

export const plan = (): TestingPlan => ({
  generator: {
    id: "counter-actions",
    version: "1",
    initial: ["s"],
    states: {
      s: ["noise", "add", "crash", "restart"].map((action) => ({
        action,
        target: "s",
      })),
    },
    minLength: 0,
    maxLength: 24,
  },
  seed: 0x36a1,
  cases: 64,
  maxAttempts: 128,
  maxShrinkChecks: 128,
  maxWork: 100_000,
});
export const registerContract = (
  test: (name: string, run: () => void | Promise<void>) => unknown,
): void => {
  test("portable property sessions require valid cases and preserve unknown and host errors", async () => {
    const runtime = await createTestingWasm(
      await readFile(
        "crates/idealis-connect/target/wasm32-unknown-unknown/release/idealis_connect.wasm",
      ),
    );
    const passed = await runtime.property(plan(), () => ({
      status: "matched",
    }));
    assert.equal(passed.status, "passed");
    assert.equal(passed.cases, 64);
    const rejected = await runtime.property(plan(), () => ({
      status: "invalid",
    }));
    assert.equal(rejected.status, "exhausted");
    assert.equal(rejected.cases, 0);
    assert.equal(
      (
        await runtime.property({ ...plan(), maxWork: 0 }, () => ({
          status: "matched",
        }))
      ).status,
      "inconclusive",
    );
    const error = new Error("driver setup failed");
    await assert.rejects(
      runtime.property(plan(), () => {
        throw error;
      }),
      (actual) => actual === error,
    );
    assert.equal(
      (
        await runtime.property({ ...plan(), cases: 1 }, () => ({
          status: "matched",
        }))
      ).status,
      "passed",
    );
    await assert.rejects(
      runtime.property({ ...plan(), seed: 0 }, () => ({ status: "matched" })),
      TestingError,
    );
    assert.ok(Object.isFrozen(passed.trials));
  });
  test("native and Wasm agree with TS generation, shrinking, differential testing and real counter replay", async () => {
    const records: unknown[] = [];
    const checked = await verify([0x36a1, 0x36a2], (record) =>
      records.push(record),
    );
    assert.equal(checked.detected, 2);
    assert.equal(records.length, 2);
  });
  test("a callback can compare values while nested property sessions are rejected", async () => {
    const runtime = await createTestingWasm(
      await readFile(
        "crates/idealis-connect/target/wasm32-unknown-unknown/release/idealis_connect.wasm",
      ),
    );
    const result = await runtime.property({ ...plan(), cases: 1 }, async () => {
      await assert.rejects(
        runtime.property(plan(), () => ({ status: "matched" })),
        TestingError,
      );
      assert.equal(
        (
          await runtime.compare({ kind: "allowed", values: [{ a: 1, b: 2 }] }, [
            { id: "same", outcome: { kind: "value", value: { b: 2, a: 1 } } },
          ])
        ).status,
        "matched",
      );
      return { status: "matched" };
    });
    assert.equal(result.status, "passed");
    const error = await runtime.compare({ kind: "unknown", reason: "work" }, [
      { id: "broken", outcome: { kind: "error", message: "setup" } },
    ]);
    assert.equal(error.status, "error");
    assert.deepEqual(error.comparisons[0]?.outcome, {
      kind: "error",
      message: "setup",
    });
    assert.equal(
      (
        await runtime.compare({ kind: "allowed", values: [] }, [
          { id: "target", outcome: { kind: "value", value: null } },
        ])
      ).status,
      "invalid",
    );
    await assert.rejects(
      runtime.compare({ kind: "allowed", values: [1] }, []),
      TestingError,
    );
    await assert.rejects(
      runtime.compare({ kind: "allowed", values: [Number.NaN] }, [
        { id: "target", outcome: { kind: "value", value: 1 } },
      ]),
      TestingError,
    );
  });
};
