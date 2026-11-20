import assert from "node:assert/strict";
import type { Trace } from "../../packages/idealis/src/contracts.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

// This test projects TypedChoice's ITF records. Decimal strings preserve Quint integers.
function record(value: unknown, keys: string[], path: string) {
  assert.ok(isRecord(value), path);
  assert.deepEqual(Object.keys(value).sort(), keys, path);
  return value;
}

function decimal(value: unknown, path: string): string {
  const text = record(value, ["#bigint"], path)["#bigint"];
  assert.ok(
    typeof text === "string" &&
      /^-?\d+$/.test(text) &&
      BigInt(text).toString() === text,
    path,
  );
  return text;
}

function readState(state: unknown, index: number) {
  const path = `state ${index}`;
  assert.ok(isRecord(state), path);
  const command = record(state.command, ["tag", "value"], `${path} command`);
  const observation = record(
    state.observation,
    ["value"],
    `${path} observation`,
  );
  return {
    command,
    observation: decimal(observation.value, `${path} observation.value`),
  };
}

export function choiceTrace(dump: unknown): Trace<string, string> {
  assert.ok(isRecord(dump) && Array.isArray(dump.vars));
  assert.ok(
    dump.vars.includes("command") && dump.vars.includes("observation"),
    "missing model variables",
  );
  assert.ok(
    Array.isArray(dump.states) && dump.states.length > 0,
    "empty or missing ITF states",
  );
  const first = readState(dump.states[0], 0);
  assert.deepEqual(
    first.command,
    { tag: "Init", value: { "#tup": [] } },
    "initial command",
  );
  assert.equal(first.observation, "0", "state 0 violates storedSelection");
  const steps = dump.states.slice(1).map((state: unknown, i: number) => {
    const { command, observation } = readState(state, i + 1);
    assert.equal(command.tag, "Put", `state ${i + 1}: expected Put`);
    const input = record(
      command.value,
      ["selected"],
      `state ${i + 1} Put payload`,
    );
    const action = decimal(input.selected, `state ${i + 1} Put.selected`);
    assert.equal(
      observation,
      action,
      `state ${i + 1} violates storedSelection`,
    );
    return { action, observation };
  });
  return {
    version: 1,
    model: "TypedChoice",
    initial: first.observation,
    steps,
  };
}
