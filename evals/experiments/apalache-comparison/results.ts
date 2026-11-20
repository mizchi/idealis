import assert from "node:assert/strict";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import type { Case } from "./cases.js";

export const classify = (
  exitCode: number | null,
  log: string,
): "bounded-no-counterexample" | "counterexample" | "error" => {
  if (
    exitCode === 0 &&
    /The outcome is: NoError/.test(log) &&
    /Checker reports no error up to computation length \d+/.test(log)
  )
    return "bounded-no-counterexample";
  if (
    exitCode === 12 &&
    /The outcome is: Error/.test(log) &&
    /state invariant \d+ violated/.test(log)
  )
    return "counterexample";
  return "error";
};
const integer = (value: unknown): number => {
  assert.ok(isRecord(value) && typeof value["#bigint"] === "string");
  const number = Number(value["#bigint"]);
  assert.ok(
    Number.isSafeInteger(number) && String(number) === value["#bigint"],
  );
  return number;
};
export const itfStates = (
  value: unknown,
  family: Case["family"],
): readonly unknown[] => {
  assert.ok(
    isRecord(value) && Array.isArray(value.states) && value.states.length > 0,
  );
  assert.deepEqual(value.vars, [family === "ring" ? "x" : "f"]);
  return value.states.map((state: unknown) => {
    assert.ok(isRecord(state));
    if (family === "ring") return integer(state.x);
    assert.ok(isRecord(state.f) && Array.isArray(state.f["#map"]));
    const pairs = state.f["#map"]
      .map((pair: unknown) => {
        assert.ok(Array.isArray(pair) && pair.length === 2);
        return { key: integer(pair[0]), value: integer(pair[1]) };
      })
      .sort((left, right) => left.key - right.key);
    assert.deepEqual(
      pairs.map((pair) => pair.key),
      [1, 2, 3, 4, 5, 6, 7, 8],
    );
    return pairs.map((pair) => pair.value);
  });
};
export const validateWitness = (
  input: Case,
  states: readonly unknown[],
): void => {
  assert.ok(
    input.fault && states.length > 0 && states.length - 1 <= input.depth,
  );
  if (input.family === "ring") {
    const values = states.map((state) => {
      assert.ok(
        typeof state === "number" &&
          Number.isInteger(state) &&
          state >= 0 &&
          state < 16,
      );
      return state;
    });
    assert.equal(values[0], 0);
    for (let index = 1; index < values.length; index++)
      assert.equal(values[index], ((values[index - 1] ?? -1) + 1) % 16);
    assert.ok((values.at(-1) ?? -1) >= 8);
    assert.ok(values.slice(0, -1).every((value) => value < 8));
    return;
  }
  const values = states.map((state) => {
    assert.ok(
      Array.isArray(state) &&
        state.length === 8 &&
        state.every((value: unknown) => value === 0 || value === 1),
    );
    return state;
  });
  for (let index = 1; index < values.length; index++) {
    const previous = values[index - 1];
    assert.ok(previous);
    assert.equal(
      values[index]?.filter(
        (value: unknown, slot: number) => value !== previous[slot],
      ).length,
      1,
    );
  }
  assert.equal(values.at(-1)?.[0], 1);
  assert.ok(values.slice(0, -1).every((value) => value[0] === 0));
};
