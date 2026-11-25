import assert from "node:assert/strict";
import { test } from "node:test";
import { check } from "fast-check";
import type { BoundaryAdapter } from "./generated/withdrawal-boundary.js";
import {
  boundaryCases,
  boundaryProperty,
} from "./generated/withdrawal-boundary.js";
import {
  commandFromJson,
  configFromJson,
  stateToJson,
} from "./generated/withdrawal.js";
import type { Domain, State } from "./generated/withdrawal.js";

function account(corruptRejection: boolean): Domain {
  let state: State = {
    balance: { units: 2 },
    beforeBalance: { units: 2 },
    requested: { units: 0 },
    result: "Idle",
  };
  return {
    get state() {
      return state;
    },
    apply(command) {
      if (command.kind === "requestWithdrawal") {
        if (state.result === "Pending" || command.amount.units <= 0)
          return false;
        state = { ...state, requested: command.amount, result: "Pending" };
        return true;
      }

      if (state.result !== "Pending") return false;
      const remaining = state.balance.units - state.requested.units;
      if (command.kind === "withdraw" && remaining >= 0) {
        state = {
          ...state,
          beforeBalance: state.balance,
          balance: { units: remaining },
          result: "Withdrawn",
        };
        return true;
      }
      if (command.kind === "rejectWithdrawal" && remaining < 0) {
        state = {
          ...state,
          beforeBalance: state.balance,
          balance: corruptRejection ? { units: 0 } : state.balance,
          result: "InsufficientFunds",
        };
        return true;
      }
      return false;
    },
  };
}

function create(config: unknown, faulty: boolean): BoundaryAdapter {
  configFromJson(config);
  const domain = account(faulty);
  return {
    apply: (command) => domain.apply(commandFromJson(command)),
    observe: () => stateToJson(domain.state),
  };
}

test("the generated property runs all withdrawal boundary pairs on the independent implementation", () => {
  const property = boundaryProperty((config) => create(config, false));
  const runs = check(property, {
    seed: 113,
    numRuns: 50,
    examples: [[boundaryCases.pairs]],
  });
  assert.equal(
    runs.failed,
    false,
    runs.failed ? String(runs.errorInstance) : undefined,
  );
});

test("the generated property shrinks a corrupt rejection to a single reproducible boundary pair", () => {
  const seeded = { seed: 113, numRuns: 50 };
  const property = boundaryProperty((config) => create(config, true));
  const runs = check(property, seeded);
  assert.equal(runs.failed, true);
  assert.match(String(runs.errorInstance), /boundary for rejectWithdrawal/);
  const counterexample = runs.counterexample?.[0];
  assert.equal(counterexample?.length, 1);
  assert.equal(counterexample?.[0]?.action, "rejectWithdrawal");
  assert.ok(runs.counterexamplePath);
  const replay = check(property, {
    ...seeded,
    path: runs.counterexamplePath,
    endOnFailure: true,
  });
  assert.deepEqual(replay.counterexample, runs.counterexample);
});
