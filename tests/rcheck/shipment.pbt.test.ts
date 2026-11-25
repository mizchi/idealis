import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { check, constantFrom, property } from "fast-check";
import type { Domain, State } from "./generated/shipment.js";
import {
  commandFromJson,
  configFromJson,
  stateToJson,
} from "./generated/shipment.js";

function order(overwriteCarrier: boolean): Domain {
  let state: State = { status: { tag: "Ready" }, acknowledged: false };
  return {
    get state() {
      return state;
    },
    apply(command) {
      if (command.kind === "ship") {
        if (state.status.tag !== "Ready") return false;
        state = { ...state, status: { tag: "Shipped", value: command.method } };
        return true;
      }
      if (state.status.tag !== "Shipped") return false;
      state = {
        acknowledged: true,
        status: overwriteCarrier
          ? { tag: "Shipped", value: "Post" }
          : state.status,
      };
      return true;
    },
  };
}

interface Frame {
  "#meta": { index: number };
  config: unknown;
  command: unknown;
  observation: unknown;
}

function load(method: string): readonly Frame[] {
  const trace = JSON.parse(
    readFileSync(`fixtures/rcheck/shipment/${method}.itf.json`, "utf8"),
  );
  assert.equal(trace["#meta"].format, "ITF");
  assert.deepEqual(trace.vars, ["config", "command", "observation"]);
  assert.ok(Array.isArray(trace.states) && trace.states.length >= 2);
  return trace.states;
}

function replay([initial, ...steps]: readonly Frame[], overwrite: boolean) {
  assert.ok(initial);
  assert.equal(initial["#meta"].index, 0);
  assert.deepEqual(initial.command, { kind: "init" });
  configFromJson(initial.config);
  const implementation = order(overwrite);
  assert.deepEqual(stateToJson(implementation.state), initial.observation);
  for (const [offset, frame] of steps.entries()) {
    const index = offset + 1;
    assert.equal(frame["#meta"].index, index);
    assert.deepEqual(frame.config, initial.config);
    const command = commandFromJson(frame.command);
    assert.equal(implementation.apply(command), true, command.kind);
    const actual = stateToJson(implementation.state);
    assert.deepEqual(
      actual,
      frame.observation,
      `step ${index} ${command.kind}`,
    );
  }
}

test("the shipment PBT replays the model ITF and keeps the Courier regression", () => {
  const courier = load("courier");
  const post = load("post");
  const run = (overwrite: boolean) =>
    check(
      property(constantFrom(post, courier), (trace) =>
        replay(trace, overwrite),
      ),
      {
        seed: 118,
        numRuns: 20,
        examples: [[courier], [post]],
      },
    );
  assert.equal(run(false).failed, false);
  replay(post, true);
  const failure = run(true);
  assert.equal(failure.failed, true);
  assert.deepEqual(failure.counterexample, [courier]);
  assert.match(String(failure.errorInstance), /step 2 acknowledge/);
});
