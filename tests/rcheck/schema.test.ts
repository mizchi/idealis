// Plan 0102: ajv, an independent JSON Schema validator, agrees with
// rcheck's validator: every reachable state is accepted and the three
// states breaking a multiplicity are rejected.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import Ajv2020Module from "ajv/dist/2020.js";

const Ajv2020 = Ajv2020Module.default ?? Ajv2020Module;
const read = async (path: string): Promise<unknown> =>
  JSON.parse(
    await readFile(
      new URL(`../../../fixtures/rcheck/${path}`, import.meta.url),
      "utf8",
    ),
  );

const validators = async (name: string) => {
  const ajv = new Ajv2020({ strict: true });
  const schema = (await read(`${name}.schema.json`)) as object;
  ajv.addSchema(schema, name);
  const state = ajv.getSchema(`${name}#/$defs/State`);
  assert.ok(state, `${name}: no State`);
  return state;
};

test("ajv accepts every reachable state", async () => {
  for (const name of ["permissions", "sessions", "bank"]) {
    const valid = await validators(name);
    const states = (await read(`${name}.states.json`)) as unknown[];
    assert.ok(states.length > 0);
    for (const state of states) {
      assert.ok(
        valid(state),
        `${name}: ${JSON.stringify(state)} ${JSON.stringify(valid.errors)}`,
      );
    }
  }
});

test("ajv rejects the states breaking a multiplicity", async () => {
  const broken = (await read("broken.json")) as {
    model: string;
    why: string;
    state: unknown;
  }[];
  assert.equal(broken.length, 3);
  for (const { model, why, state } of broken) {
    const valid = await validators(model);
    assert.equal(valid(state), false, why);
  }
});
