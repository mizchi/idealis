import assert from "node:assert/strict";
import { cases } from "../single-model/cases.js";

export const inputs = () => {
  const sizes = [24, 16, 5, 100000];
  const counts = [
    [331776, 1327105],
    [65536, 1048577],
    [32768, 245761],
    [100000, 100001],
  ];
  return cases("large").map((input, index) => {
    const size = sizes[index];
    const count = counts[index];
    assert.ok(size && count?.[0] && count[1]);
    return {
      ...input,
      config: input.config.replace(
        /CONSTANT ([MN])=\d+/,
        `CONSTANT $1=${size}`,
      ),
      states: count[0],
      generated: count[1],
    };
  });
};
