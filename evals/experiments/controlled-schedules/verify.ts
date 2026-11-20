import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  createTestingWasm,
  type TestingJson,
} from "../../../packages/idealis/src/testing.js";
import { createNative, wasmPath } from "../testing-core/clients.js";
import { createSut } from "./clients.js";
import {
  completeOrders,
  graphOrders,
  plan,
  saved,
  schedule,
} from "./fixture.js";

export const verify = async (
  seeds: readonly number[],
  record: (value: unknown) => void,
) => {
  const native = createNative();
  const wasm = await createTestingWasm(await readFile(wasmPath));
  const sut = createSut();
  let detected = 0;
  let missed = 0;
  let normal = 0;
  const exhaustive: {
    actors: number;
    schedules: number;
    outcomes: number[];
    detected: number;
  }[] = [];
  try {
    for (const actors of [2, 3]) {
      const input = schedule(actors);
      const generator = await native.runtime.schedule(input);
      assert.deepEqual(await wasm.schedule(input), generator);
      const orders = completeOrders(actors);
      assert.equal(orders.length, actors === 2 ? 6 : 90);
      const ordered = (values: readonly (readonly TestingJson[])[]) =>
        [...values].sort((left, right) =>
          JSON.stringify(left, ["actor", "operation", "step"]).localeCompare(
            JSON.stringify(right, ["actor", "operation", "step"]),
          ),
        );
      assert.deepEqual(ordered(graphOrders(generator)), ordered(orders));
      const outcomes = new Set<number>();
      let failures = 0;
      for (const actions of orders) {
        const testCase = saved(actors, actions);
        const fixed = await sut.evaluate(actors, testCase, false);
        const broken = await sut.evaluate(actors, testCase, true);
        assert.equal(fixed.verdict.status, "matched");
        outcomes.add(broken.actual);
        failures += Number(broken.verdict.status === "mismatch");
        record({ kind: "complete-order", actors, testCase, fixed, broken });
      }
      const serialized = saved(
        actors,
        input.actors.flatMap((actor) =>
          actor.operations.map((operation, step) => ({
            actor: actor.id,
            step,
            operation,
          })),
        ),
      );
      assert.equal(
        (await sut.evaluate(actors, serialized, true)).verdict.status,
        "matched",
      );
      assert.ok(failures > 0);
      exhaustive.push({
        actors,
        schedules: orders.length,
        outcomes: [...outcomes].sort(),
        detected: failures,
      });
      for (const seed of seeds) {
        for (const broken of [false, true]) {
          const run = async (testCase: Parameters<typeof sut.evaluate>[1]) =>
            (await sut.evaluate(actors, testCase, broken)).verdict;
          const result = await native.runtime.property(
            plan(generator, seed),
            run,
          );
          const mirrored = await wasm.property(plan(generator, seed), run);
          assert.deepEqual(mirrored, result);
          if (!broken) {
            assert.equal(result.status, "passed");
            normal++;
          }
          let reproduction: unknown = null;
          if (broken && result.counterexample) {
            detected++;
            const testCase = result.counterexample;
            const fault = await sut.evaluate(actors, testCase, true);
            const fixed = await sut.evaluate(actors, testCase, false);
            assert.equal(fault.verdict.status, "mismatch");
            assert.equal(fixed.verdict.status, "matched");
            assert.equal(
              (await wasm.replay(testCase, run, "lost-update")).status,
              "failed",
            );
            assert.equal(
              (
                await native.runtime.replay(
                  testCase,
                  async (value) =>
                    (await sut.evaluate(actors, value, false)).verdict,
                )
              ).status,
              "passed",
            );
            let checkedDeletions = 0;
            if (result.shrink?.status === "minimal") {
              for (let index = 0; index < testCase.actions.length; index++) {
                const candidate = {
                  ...testCase,
                  actions: testCase.actions.filter(
                    (_, position) => position !== index,
                  ),
                };
                const evaluated = await sut.evaluate(actors, candidate, true);
                assert.notEqual(evaluated.verdict.status, "mismatch");
                checkedDeletions++;
              }
            }
            reproduction = { fault, fixed, checkedDeletions };
          } else if (broken) {
            assert.equal(result.status, "passed");
            missed++;
          }
          record({
            kind: "property",
            actors,
            seed,
            broken,
            result,
            reproduction,
          });
        }
      }
      let called = false;
      assert.equal(
        (
          await wasm.property(
            { ...plan(generator, 0x3801), maxWork: 0 },
            () => {
              called = true;
              return { status: "matched" };
            },
          )
        ).status,
        "inconclusive",
      );
      assert.equal(called, false);
    }
    return {
      seeds: seeds.length,
      normal,
      detected,
      missed,
      exhaustive,
      exchanges: native.exchanges,
      sutExchanges: sut.exchanges,
    };
  } finally {
    await sut.close();
    await native.close();
  }
};
