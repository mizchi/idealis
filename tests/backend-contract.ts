import assert from "node:assert/strict";
import {
  BackendContractError,
  completeInduction,
  parseBackendResult,
} from "../packages/idealis/src/backend.js";
import type {
  BackendOutcome,
  BackendQuery,
  BackendResult,
  BackendScope,
} from "../packages/idealis/src/contracts.js";

const scope: BackendScope = {
  modelId: "ring:v1",
  propertyId: "Safe:v1",
  semantics: "tlc-1.7.4-int32",
  assumptions: [],
};
const result = (
  query: BackendQuery,
  outcome: BackendOutcome,
  initialization: BackendResult["initialization"] = "nonempty",
): BackendResult => ({
  format: "modelt.backend-result",
  version: 1,
  backend: { id: "checker", version: "1" },
  scope,
  query,
  initialization,
  outcome,
});
const obligation = (
  name: "init" | "step" | "implication",
  invariantId = scope.propertyId,
): BackendResult =>
  result(
    { kind: "induction", invariantId, obligation: name },
    { kind: "obligation-holds" },
    name === "init" ? "nonempty" : "unknown",
  );

export const registerBackendContract = (
  test: (name: string, run: () => void | Promise<void>) => unknown,
): void => {
  test("behavior witnesses keep state arity, initial actions, loop edges and bound semantics", () => {
    const witness = {
      kind: "behavior",
      violation: "liveness-violation",
      name: "Eventually",
      variables: ["x"],
      trace: [{ action: null, values: [{ type: "int", value: "0" }] }],
      loopStart: 0,
      loopAction: "Next",
    } as const;
    const valid = result(
      { kind: "bounded", maxDepth: 1 },
      { kind: "counterexample", witness },
    );
    assert.deepEqual(parseBackendResult(valid), valid);
    for (const invalid of [
      { ...valid, query: { kind: "bounded", maxDepth: 0 } },
      {
        ...valid,
        outcome: {
          kind: "counterexample",
          witness: { ...witness, loopStart: 1 },
        },
      },
      {
        ...valid,
        outcome: {
          kind: "counterexample",
          witness: { ...witness, variables: ["x", "y"] },
        },
      },
      {
        ...valid,
        outcome: {
          kind: "counterexample",
          witness: {
            ...witness,
            trace: [{ action: "Next", values: [{ type: "int", value: "0" }] }],
          },
        },
      },
      {
        ...valid,
        outcome: {
          kind: "counterexample",
          witness: {
            kind: "obligation",
            assignment: { type: "record", value: [] },
          },
        },
      },
    ])
      assert.throws(() => parseBackendResult(invalid), BackendContractError);
  });
  test("backend snapshots isolate metadata and reject forged induction evidence", () => {
    const original = JSON.parse(JSON.stringify(obligation("init")));
    const parsed = parseBackendResult(original);
    original.scope.modelId = "changed";
    original.scope.assumptions.push("changed");
    assert.equal(parsed.scope.modelId, scope.modelId);
    assert.deepEqual(parsed.scope.assumptions, []);
    const aggregate = completeInduction([
      obligation("init"),
      obligation("step"),
    ]);
    assert.deepEqual(
      parseBackendResult(JSON.parse(JSON.stringify(aggregate))),
      aggregate,
    );
    if (aggregate.outcome.kind !== "inductive")
      throw new Error("Expected proof");
    assert.throws(
      () =>
        parseBackendResult({
          ...aggregate,
          outcome: {
            kind: "inductive",
            obligations: [
              { ...obligation("init"), initialization: "unknown" },
              obligation("step"),
            ],
          },
        }),
      BackendContractError,
    );
    assert.throws(
      () =>
        parseBackendResult({
          ...aggregate,
          outcome: { kind: "inductive", obligations: [obligation("init")] },
        }),
      BackendContractError,
    );
    assert.throws(
      () =>
        parseBackendResult({
          ...parsed,
          scope: { ...scope, modelId: "\ud800" },
        }),
      BackendContractError,
    );
  });
  test("backend wire contracts preserve guarantees and reject unsupported versions and claims", () => {
    const bounded = result(
      { kind: "bounded", maxDepth: 0 },
      { kind: "bounded-no-counterexample", maxDepth: 0 },
    );
    assert.deepEqual(
      parseBackendResult(JSON.parse(JSON.stringify(bounded))),
      bounded,
    );
    for (const invalid of [
      { ...bounded, version: 2 },
      { ...bounded, extra: true },
      {
        ...bounded,
        outcome: {
          kind: "inconclusive",
          reason: { toString: () => "solver-unknown" },
        },
      },
      { ...bounded, outcome: { kind: "inconclusive", reason: "obligations" } },
      { ...bounded, initialization: "empty" },
      { ...bounded, initialization: "unknown" },
      { ...bounded, outcome: { kind: "explicit-complete", states: 1 } },
      {
        ...bounded,
        outcome: { kind: "bounded-no-counterexample", maxDepth: 1 },
      },
    ])
      assert.throws(() => parseBackendResult(invalid), BackendContractError);
    const complete = result(
      {
        kind: "explicit",
        bounds: { maxDepth: 10, maxStates: 100, maxWork: 1000 },
      },
      { kind: "explicit-complete", states: 4 },
    );
    assert.deepEqual(parseBackendResult(complete), complete);
    assert.throws(
      () =>
        parseBackendResult({
          ...complete,
          outcome: { kind: "explicit-complete", states: 0 },
        }),
      BackendContractError,
    );
  });
  test("induction requires matching obligations, implication and a nonempty initial domain", () => {
    const proven = completeInduction([obligation("step"), obligation("init")]);
    assert.equal(proven.outcome.kind, "inductive");
    assert.equal(
      completeInduction([obligation("init")]).outcome.kind,
      "inconclusive",
    );
    assert.equal(
      completeInduction([
        obligation("init", "Stronger"),
        obligation("step", "Stronger"),
      ]).outcome.kind,
      "inconclusive",
    );
    assert.equal(
      completeInduction([
        obligation("init", "Stronger"),
        obligation("step", "Stronger"),
        obligation("implication", "Stronger"),
      ]).outcome.kind,
      "inductive",
    );
    for (const initialization of ["unknown", "empty"] as const)
      assert.equal(
        completeInduction([
          { ...obligation("init"), initialization },
          obligation("step"),
        ]).outcome.kind,
        "inconclusive",
      );
    for (const changed of [
      { ...obligation("step"), scope: { ...scope, modelId: "other" } },
      { ...obligation("step"), scope: { ...scope, propertyId: "other" } },
      { ...obligation("step"), scope: { ...scope, semantics: "smt-int" } },
      {
        ...obligation("step"),
        scope: { ...scope, assumptions: ["trusted-network"] },
      },
      { ...obligation("step"), backend: { id: "checker", version: "2" } },
    ])
      assert.throws(
        () => completeInduction([obligation("init"), changed]),
        BackendContractError,
      );
    assert.throws(
      () => completeInduction([obligation("init"), obligation("init")]),
      BackendContractError,
    );
  });
  test("unknown, errors and obligation counterexamples never become an inductive proof", () => {
    for (const outcome of [
      { kind: "inconclusive", reason: "solver-unknown" },
      { kind: "error", code: "solver", message: "failure" },
      {
        kind: "counterexample",
        witness: {
          kind: "obligation",
          assignment: { type: "record", value: [] },
        },
      },
    ] satisfies readonly BackendOutcome[]) {
      const incomplete = completeInduction([
        obligation("init"),
        { ...obligation("step"), outcome },
      ]);
      assert.equal(incomplete.outcome.kind, "inconclusive");
      assert.ok(
        incomplete.outcome.kind === "inconclusive" &&
          incomplete.outcome.obligations?.some(
            (entry) => entry.outcome.kind === outcome.kind,
          ),
      );
    }
    assert.throws(
      () =>
        parseBackendResult({
          ...obligation("step"),
          outcome: {
            kind: "counterexample",
            witness: {
              kind: "behavior",
              violation: "invariant-violation",
              name: "Safe",
              variables: ["x"],
              trace: [{ action: null, values: [{ type: "int", value: "0" }] }],
            },
          },
        }),
      BackendContractError,
    );
  });
};
