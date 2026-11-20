import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createSut } from "../../evals/experiments/controlled-schedules/clients.js";
import { evaluate } from "../../evals/experiments/controlled-schedules/driver.js";
import {
  plan,
  schedule,
} from "../../evals/experiments/controlled-schedules/fixture.js";
import { wasmPath } from "../../evals/experiments/testing-core/clients.js";
import {
  createTestingWasm,
  TestingError,
} from "../../packages/idealis/src/testing.js";

export const register = (
  test: (name: string, run: () => Promise<void>) => unknown,
) => {
  test("stateful TS and gated Rust replay the shrunk lost-update witness", async () => {
    const runtime = await createTestingWasm(await readFile(wasmPath));
    const generator = await runtime.schedule(schedule(3));
    const sut = createSut();
    try {
      assert.equal(
        (
          await runtime.property(
            plan(generator, 0x3801),
            async (value) => (await evaluate(3, value, false)).verdict,
          )
        ).status,
        "passed",
      );
      const broken = await runtime.property(
        plan(generator, 0x3801),
        async (value) => (await sut.evaluate(3, value, true)).verdict,
      );
      assert.equal(broken.status, "failed");
      assert.equal(broken.shrink?.status, "minimal");
      assert.ok(broken.counterexample);
      assert.equal(
        (
          await runtime.replay(
            broken.counterexample,
            async (value) => (await sut.evaluate(3, value, true)).verdict,
            "lost-update",
          )
        ).status,
        "failed",
      );
      assert.equal(
        (
          await runtime.replay(
            broken.counterexample,
            async (value) => (await sut.evaluate(3, value, false)).verdict,
          )
        ).status,
        "passed",
      );
    } finally {
      await sut.close();
    }
  });
  test("schedule compilation preserves actor order and snapshots operations", async () => {
    const runtime = await createTestingWasm(await readFile(wasmPath));
    const input = {
      id: "schedule",
      version: "1",
      actors: [
        { id: "left", operations: ["read", "write"] },
        { id: "right", operations: ["read", "write"] },
      ],
    };
    const pending = runtime.schedule(input);
    input.actors[0]?.operations.push("changed");
    const generator = await pending;
    assert.equal(generator.maxLength, 4);
    assert.equal(Object.keys(generator.states).length, 9);
    assert.deepEqual(
      generator.states["0,0"]?.map((edge) => edge.action),
      [
        { actor: "left", step: 0, operation: "read" },
        { actor: "right", step: 0, operation: "read" },
      ],
    );
    assert.ok(Object.isFrozen(generator.states["0,0"]?.[0]?.action));
    assert.equal(Reflect.set(generator, "maxLength", 99), false);
  });
  test("invalid compilation leaves the active property session intact", async () => {
    const runtime = await createTestingWasm(await readFile(wasmPath));
    const input = {
      id: "one",
      version: "1",
      actors: [{ id: "actor", operations: [] }],
    };
    const generator = await runtime.schedule(input);
    let calls = 0;
    const result = await runtime.property(
      {
        generator,
        seed: 1,
        cases: 1,
        maxAttempts: 1,
        maxShrinkChecks: 16,
        maxWork: 1000,
      },
      async () => {
        calls++;
        await assert.rejects(
          runtime.schedule({ ...input, actors: [] }),
          TestingError,
        );
        assert.equal((await runtime.schedule(input)).maxLength, 0);
        return { status: "matched" };
      },
    );
    assert.equal(result.status, "passed");
    assert.equal(calls, 1);
  });
};
