import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { corpus } from "../../evals/experiments/rtla-parser/corpus.js";
import { operatorSharingCases } from "../../evals/experiments/tlc-differential/corpus.js";
import { expandTemporalTemplates } from "../../evals/experiments/tlc-differential/expand-temporal.js";
import { digest } from "../../evals/experiments/usable/provenance.js";
import type {
  SafetyDefinition,
  SafetyExpressionV2,
  SafetyProgram,
  SafetyProgramV2,
  SafetyResult,
  TlaSourceOptions,
} from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { checkSafety } from "../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import {
  checkTlaSource,
  compileTlaSource,
} from "../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import { createCiWasm } from "../ci/tlc-checkers.js";
import { native } from "./operator-sharing-support.js";

const directory = "fixtures/tlc-operator-sharing";
const bounds = { maxDepth: 64, maxStates: 10_000, maxWork: 1_000_000 };
const core = await createCiWasm();
interface Golden {
  readonly name: string;
  readonly source: string;
  readonly config: string;
  readonly log: string;
  readonly sourceSha256: string;
  readonly configSha256: string;
  readonly logSha256: string;
  readonly exit: number;
  readonly status:
    | "success"
    | "error"
    | "input-error"
    | "invariant-violation"
    | "liveness-violation";
  readonly states?: number;
  readonly generated?: number;
  readonly message?: string;
  readonly location?: string;
  readonly depth?: number;
  readonly violation?: SafetyResult["violation"];
}
const originalGolden: { readonly cases: readonly Golden[] } = JSON.parse(
  await readFile(`${directory}/expected.json`, "utf8"),
);
const additionalGolden: { readonly cases: readonly Golden[] } = JSON.parse(
  await readFile(`${directory}/expected-step3c.json`, "utf8"),
);
const generativeGolden: { readonly cases: readonly Golden[] } = JSON.parse(
  await readFile(`${directory}/expected-step3d-a.json`, "utf8"),
);
const actionFrameGolden: { readonly cases: readonly Golden[] } = JSON.parse(
  await readFile(`${directory}/expected-step3e-a.json`, "utf8"),
);
const amendedFrameGolden: { readonly cases: readonly Golden[] } = JSON.parse(
  await readFile(`${directory}/expected-step3e-b.json`, "utf8"),
);
const actionFrameV1: {
  readonly cases: readonly {
    readonly name: string;
    readonly outcome: unknown;
  }[];
} = JSON.parse(await readFile(`${directory}/v1-current-step3e-a.json`, "utf8"));
const currentBehaviorCases = [
  ...generativeGolden.cases,
  ...actionFrameGolden.cases,
];
const golden = {
  cases: [
    ...originalGolden.cases,
    ...additionalGolden.cases,
    ...generativeGolden.cases,
    ...actionFrameGolden.cases,
    ...amendedFrameGolden.cases,
  ],
};
interface Divergence {
  readonly name: string;
  readonly tlc: Pick<
    Golden,
    "exit" | "message" | "logSha256" | "states" | "generated" | "violation"
  > & { readonly kind: "liveness-violation" };
  readonly semantic: {
    readonly propertyHolds: boolean;
    readonly outcome: {
      readonly kind: "safety";
      readonly result: SafetyResult;
    };
  };
  readonly controls: readonly {
    readonly name: string;
    readonly work: number;
  }[];
}
const divergenceSource = await readFile(
  `${directory}/divergences.json`,
  "utf8",
);
const divergences: {
  readonly schemaVersion: number;
  readonly tlcRevision: string;
  readonly cases: readonly Divergence[];
} = JSON.parse(divergenceSource);
const divergenceLogs = new Map(
  await Promise.all(
    divergences.cases.map(async (record) => {
      const fixture = golden.cases.find((entry) => entry.name === record.name);
      assert.ok(fixture, record.name);
      return [
        record.name,
        await readFile(`${directory}/${fixture.log}`, "utf8"),
      ] as const;
    }),
  ),
);
const inputs = await operatorSharingCases();
const capture = <Value>(run: () => Value) => {
  try {
    return run();
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error" as const, code: error.code, message: error.message };
  }
};
const inputAt = (name: string) => {
  const input = inputs.find((entry) => entry.name === name);
  assert.ok(input, name);
  return input;
};
const versioned = (
  input: TlaSourceOptions,
  irVersion: 1 | 2,
): TlaSourceOptions => ({
  source: input.source,
  config: input.config,
  irVersion,
});
const budget = (name: string) => ({
  ...bounds,
  maxWork: name === "sharing-work-60" ? 256 : bounds.maxWork,
});
const normalize = (input: unknown): unknown => {
  const value = expandTemporalTemplates(input);
  if (Array.isArray(value)) return value.map(normalize);
  if (!isRecord(value)) return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(
        ([key, entry]) =>
          !(key === "subscript" && entry === null) &&
          !(
            [
              "fairness",
              "initialProperties",
              "actionProperties",
              "properties",
            ].includes(key) &&
            Array.isArray(entry) &&
            !entry.length
          ),
      )
      .map(([key, entry]) => [key, normalize(entry)]),
  );
};
const assertOracle = (fixture: Golden, outcome: unknown) => {
  assert.ok(isRecord(outcome), fixture.name);
  if (fixture.name === "sharing-work-60") {
    assert.equal(outcome.kind, "safety", JSON.stringify(outcome));
    assert.ok(isRecord(outcome.result));
    assert.equal(outcome.result.status, "bounded");
    assert.equal(outcome.result.reason, "work");
    assert.ok(
      typeof outcome.result.work === "number" && outcome.result.work <= 256,
    );
    return;
  }
  if (fixture.status === "error") {
    assert.equal(outcome.kind, "error", JSON.stringify(outcome));
    assert.equal(outcome.code, "protocol");
    assert.equal(typeof outcome.message, "string");
    assert.match(String(outcome.message), /Head.*empty sequence/);
    return;
  }
  if (fixture.status === "invariant-violation") {
    assert.equal(outcome.kind, "safety", JSON.stringify(outcome));
    assert.ok(isRecord(outcome.result));
    assert.equal(outcome.result.status, fixture.status);
    assert.equal(outcome.result.reason, null);
    assert.equal(outcome.result.depth, fixture.depth ?? 0);
    if (fixture.states !== undefined)
      assert.equal(outcome.result.states, fixture.states);
    if (fixture.generated !== undefined)
      assert.equal(outcome.result.generated, fixture.generated);
    assert.ok(fixture.violation);
    if ((fixture.depth ?? 0) === 0)
      assert.deepEqual(outcome.result.violation, fixture.violation);
    else {
      assert.ok(isRecord(outcome.result.violation));
      assert.equal(outcome.result.violation.name, fixture.violation.name);
      assert.ok(Array.isArray(outcome.result.violation.trace));
      assert.deepEqual(
        outcome.result.violation.trace.map((step: unknown) => {
          assert.ok(isRecord(step));
          return step.values;
        }),
        fixture.violation.trace.map((step) => step.values),
      );
    }
    return;
  }
  if (fixture.status === "liveness-violation") {
    assert.equal(outcome.kind, "safety", JSON.stringify(outcome));
    assert.ok(isRecord(outcome.result));
    assert.equal(outcome.result.status, fixture.status);
    assert.equal(outcome.result.reason, null);
    assert.equal(outcome.result.states, fixture.states);
    assert.equal(outcome.result.generated, fixture.generated);
    assert.deepEqual(outcome.result.violation, fixture.violation);
    return;
  }
  assert.equal(fixture.status, "success");
  assert.equal(outcome.kind, "safety", JSON.stringify(outcome));
  assert.ok(isRecord(outcome.result));
  assert.equal(outcome.result.status, "complete");
  assert.equal(outcome.result.states, fixture.states);
  assert.equal(outcome.result.generated, fixture.generated);
  assert.equal(outcome.result.violation, null);
};
const assertDivergenceRecord = (
  record: Divergence,
  fixture: Golden,
  log: string,
) => {
  assert.equal(record.name, fixture.name);
  assert.equal(fixture.status, "liveness-violation");
  assert.equal(fixture.exit, 13);
  assert.deepEqual(record.tlc, {
    kind: fixture.status,
    exit: fixture.exit,
    message: fixture.message,
    logSha256: fixture.logSha256,
    states: fixture.states,
    generated: fixture.generated,
    violation: fixture.violation,
  });
  assert.equal(digest(log), record.tlc.logSha256);
  assert.match(log, /Error: Temporal properties were violated\./);
  assert.match(log, /State 1: <Initial predicate>\nx = 0\n/);
  assert.match(log, /State 2: Stuttering\n/);
  assert.equal((log.match(/State \d+:/g) ?? []).length, 2);
  assert.ok(
    log.includes(
      `${record.tlc.generated} states generated, ${record.tlc.states} distinct states found`,
    ),
  );
  assert.equal(record.semantic.propertyHolds, true);
  assert.equal(record.semantic.outcome.kind, "safety");
  assert.equal(record.semantic.outcome.result.status, "complete");
  assert.equal(record.semantic.outcome.result.reason, null);
  assert.equal(record.semantic.outcome.result.violation, null);
};
const assertV2Oracle = (fixture: Golden, outcome: unknown) => {
  const record = divergences.cases.find((entry) => entry.name === fixture.name);
  if (record === undefined) {
    assertOracle(fixture, outcome);
    return;
  }
  const log = divergenceLogs.get(record.name);
  assert.ok(log);
  assertDivergenceRecord(record, fixture, log);
  assert.deepEqual(outcome, record.semantic.outcome);
};

test("operator sharing recorded TLC divergence and direct controls", async () => {
  assert.equal(
    digest(divergenceSource),
    "0853cfc3291ef85e4b26bc6d1833f8e2e24bac337582188861827aaafef1e31b",
  );
  assert.equal(divergences.schemaVersion, 1);
  assert.equal(
    divergences.tlcRevision,
    "5a47802b5c391f59ecdd44117981f4ff8c0656ba",
  );
  assert.deepEqual(
    divergences.cases.map((record) => record.name),
    ["sharing-action-frame-temporal"],
  );
  for (const record of divergences.cases) {
    assert.deepEqual(
      record.controls.map((control) => control.name),
      ["sharing-temporal-true-step3e-b", "sharing-temporal-forall-step3e-b"],
    );
    for (const entry of [
      { name: record.name, outcome: record.semantic.outcome },
      ...record.controls.map((control) => ({
        name: control.name,
        outcome: {
          kind: "safety",
          result: { ...record.semantic.outcome.result, work: control.work },
        },
      })),
    ]) {
      const fixture = golden.cases.find(
        (candidate) => candidate.name === entry.name,
      );
      assert.ok(fixture);
      if (entry.name !== record.name) {
        assert.equal(fixture.status, "success");
        assert.equal(fixture.exit, 0);
        const log = await readFile(`${directory}/${fixture.log}`, "utf8");
        assert.equal(digest(log), fixture.logSha256);
        assert.match(
          log,
          /Model checking completed\. No error has been found\./,
        );
        assertOracle(fixture, entry.outcome);
      }
      const input = versioned(inputAt(entry.name), 2);
      const ts = compileTlaSafety(input);
      const rust = compileTlaSource(core, input);
      assert.deepEqual(normalize(ts), normalize(rust));
      assert.deepEqual(
        { kind: "safety", result: checkSafety(core, ts, bounds) },
        entry.outcome,
      );
      assert.deepEqual(
        { kind: "safety", result: checkTlaSource(core, input, bounds) },
        entry.outcome,
      );
      for (const request of [
        { kind: "check-tla" as const, ...input, bounds },
        { kind: "check-safety" as const, program: ts, bounds },
      ]) {
        const wasm = core.request({ version: 1, request });
        const response = native(request);
        assert.deepEqual(response, wasm);
        assert.deepEqual(response, entry.outcome);
        assertV2Oracle(fixture, response);
      }
    }
  }
});

test("operator sharing divergence runner refuses a non-diverging fixture", async () => {
  const record = divergences.cases[0];
  assert.ok(record);
  const fixture = golden.cases.find(
    (entry) => entry.name === "sharing-temporal-true-step3e-b",
  );
  assert.ok(fixture);
  const log = await readFile(`${directory}/${fixture.log}`, "utf8");
  assert.throws(
    () =>
      assertDivergenceRecord({ ...record, name: fixture.name }, fixture, log),
    assert.AssertionError,
  );
});

test("operator sharing divergence runner refuses a changed log or violation", () => {
  const record = divergences.cases[0];
  assert.ok(record);
  const fixture = golden.cases.find((entry) => entry.name === record.name);
  const log = divergenceLogs.get(record.name);
  assert.ok(fixture && log);
  assert.throws(
    () =>
      assertDivergenceRecord(record, fixture, log.replace("x = 0", "x = 1")),
    assert.AssertionError,
  );
  assert.throws(
    () =>
      assertDivergenceRecord(
        { ...record, tlc: { ...record.tlc, exit: 0 } },
        fixture,
        log,
      ),
    assert.AssertionError,
  );
});

for (const fixture of golden.cases)
  test(`operator sharing fixture: ${fixture.name}`, async () => {
    const source = await readFile(`${directory}/${fixture.source}`, "utf8");
    const config = await readFile(`${directory}/${fixture.config}`, "utf8");
    const log = await readFile(`${directory}/${fixture.log}`, "utf8");
    assert.equal(digest(source), fixture.sourceSha256);
    assert.equal(digest(config), fixture.configSha256);
    assert.equal(digest(log), fixture.logSha256);
    assert.doesNotMatch(
      log,
      /\/Users\/|\/private\/|\/var\/folders|\[pid: \d+\]|\bon \d+ cores\b/,
    );
    if (fixture.status === "success") {
      assert.equal(fixture.exit, 0);
      assert.match(log, /Model checking completed\. No error has been found\./);
    } else if (fixture.status === "invariant-violation") {
      assert.equal(fixture.exit, 12);
      if ((fixture.depth ?? 0) === 0) {
        assert.equal(
          fixture.message,
          "Invariant Safe is violated by the initial state:",
        );
        assert.ok(log.includes(`${fixture.message}\nx = 0\n`));
        assert.deepEqual(fixture.violation, {
          name: "Safe",
          trace: [{ action: null, values: [{ type: "int", value: "0" }] }],
        });
      } else {
        assert.equal(fixture.message, "Invariant Safe is violated.");
        assert.ok(log.includes(fixture.message));
        assert.match(log, /State 1: <Initial predicate>\nx = 0\n/);
        assert.match(log, /State 2: <Step [^\n]+>\nx = 1\n/);
        assert.deepEqual(fixture.violation, {
          name: "Safe",
          trace: [
            { action: null, values: [{ type: "int", value: "0" }] },
            { action: "Step", values: [{ type: "int", value: "1" }] },
          ],
        });
      }
    } else if (fixture.status === "liveness-violation") {
      assert.equal(fixture.exit, 13);
      assert.equal(fixture.message, "Temporal properties were violated.");
      assert.ok(log.includes(fixture.message));
      assert.match(log, /State 1: <Initial predicate>\nx = 0\n/);
      assert.match(log, /State 2: Stuttering/);
      assert.deepEqual(fixture.violation, {
        name: "P",
        trace: [{ action: null, values: [{ type: "int", value: "0" }] }],
        loopStart: 0,
        loopAction: "<stuttering>",
      });
    } else {
      assert.equal(
        fixture.exit,
        fixture.name === "sharing-action-frame-temporal-state"
          ? 255
          : fixture.status === "error"
            ? 75
            : 150,
      );
      assert.ok(fixture.message && fixture.location);
      assert.ok(log.includes(fixture.message));
      assert.ok(log.includes(fixture.location));
    }
    if (fixture.states !== undefined)
      assert.ok(log.includes(`${fixture.states} distinct states found`));
    if (fixture.generated !== undefined)
      assert.ok(log.includes(`${fixture.generated} states generated`));
    if (fixture.status !== "input-error") {
      const input = inputAt(fixture.name);
      assert.equal(input.source, source);
      assert.equal(input.config, config);
      assert.equal(input.irVersion, 2);
      assert.equal(input.goldenFrontendParity, true);
    }
  });

const serialization = (run: () => SafetyProgram) => {
  try {
    return { serializedSha256: digest(JSON.stringify(run())) };
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return {
      error: { name: error.name, code: error.code, message: error.message },
    };
  }
};
interface Baseline {
  readonly revision: string;
  readonly existingInputs: number;
  readonly cases: readonly {
    readonly name: string;
    readonly sourceSha256: string;
    readonly configSha256: string;
    readonly ts: ReturnType<typeof serialization>;
    readonly rust: ReturnType<typeof serialization>;
    readonly nativeSha256: string;
  }[];
}
const baseline: Baseline = JSON.parse(
  await readFile(`${directory}/v1-baseline.json`, "utf8"),
);
const existing = await corpus();
const frozen = [
  ...existing,
  ...inputs
    .filter((input) =>
      originalGolden.cases.some((fixture) => fixture.name === input.name),
    )
    .sort((a, b) => a.name.localeCompare(b.name)),
];
test("operator sharing v1 baseline covers all 239 existing compiler inputs separately", () => {
  assert.equal(baseline.revision, "fbf0d664");
  assert.equal(existing.length, 239);
  assert.equal(baseline.existingInputs, existing.length);
  assert.equal(baseline.cases.length, existing.length + 17);
  for (const frontend of ["ts", "rust"] as const) {
    assert.equal(
      baseline.cases
        .slice(0, existing.length)
        .filter((entry) => entry[frontend].serializedSha256 !== undefined)
        .length,
      237,
    );
    assert.equal(
      baseline.cases
        .slice(0, existing.length)
        .filter((entry) => entry[frontend].error !== undefined).length,
      2,
    );
  }
});
for (const [index, input] of frozen.entries())
  test(
    `operator sharing v1 byte identity: ${input.name}`,
    {
      timeout: 60_000,
    },
    () => {
      const saved = baseline.cases[index];
      assert.ok(saved);
      assert.equal(saved.name, input.name);
      assert.equal(saved.sourceSha256, digest(input.source));
      assert.equal(saved.configSha256, digest(input.config));
      for (const irVersion of [undefined, 1] as const) {
        const selected = {
          source: input.source,
          config: input.config,
          ...(irVersion === undefined ? {} : { irVersion }),
        };
        assert.deepEqual(
          serialization(() => compileTlaSafety(selected)),
          saved.ts,
        );
        assert.deepEqual(
          serialization(() => compileTlaSource(core, selected)),
          saved.rust,
        );
        assert.equal(
          digest(JSON.stringify(native({ kind: "compile-tla", ...selected }))),
          saved.nativeSha256,
        );
      }
    },
  );

const v1Refusals = new Map([
  ...[
    "sharing-chain-20",
    "sharing-chain-30",
    "sharing-chain-60",
    "sharing-work-60",
  ].map(
    (name) =>
      [
        name,
        "Unsupported TLA input: expression or operator nesting exceeds 40",
      ] as const,
  ),
  ...[
    "sharing-lexical-capture",
    "sharing-binder-quantifier",
    "sharing-binder-function",
    "sharing-binder-filter",
    "sharing-binder-map-set",
    "sharing-binder-tuple-pattern",
    "sharing-generative-binder",
    "sharing-generative-binder-exact",
  ].map((name) => [name, "Invalid bound variable slot"] as const),
  ...["sharing-binder-fold-sequence", "sharing-binder-fold-set"].map(
    (name) => [name, "Invalid fold variable slot"] as const,
  ),
  ...[
    "sharing-generative-membership",
    "sharing-generative-membership-alias",
  ].map((name) => [name, "Unsupported TLA input: operator Nat"] as const),
]);
const assertVersionedOracle = (
  fixture: Golden,
  irVersion: 1 | 2,
  outcome: unknown,
) => {
  if (
    irVersion === 1 &&
    [
      "sharing-generative-except",
      "sharing-generative-except-control",
      "sharing-action-frame-except",
      "sharing-action-frame-except-control",
    ].includes(fixture.name)
  ) {
    assert.deepEqual(outcome, {
      kind: "error",
      code: "protocol",
      message:
        "Unsupported TLA input: EXCEPT @ arguments across a nested EXCEPT require safety IR v2",
    });
    return;
  }
  const current = actionFrameV1.cases.find(
    (entry) => entry.name === fixture.name,
  );
  if (irVersion === 1 && current !== undefined) {
    assert.deepEqual(outcome, current.outcome);
    return;
  }
  const refusal = irVersion === 1 ? v1Refusals.get(fixture.name) : undefined;
  if (refusal !== undefined) {
    assert.deepEqual(outcome, {
      kind: "error",
      code: "protocol",
      message: refusal,
    });
  } else if (irVersion === 2) assertV2Oracle(fixture, outcome);
  else assertOracle(fixture, outcome);
};

for (const input of inputs) {
  const fixture = golden.cases.find((entry) => entry.name === input.name);
  assert.ok(fixture);
  const amended = amendedFrameGolden.cases.some(
    (entry) => entry.name === input.name,
  );
  const versions: readonly (1 | 2)[] = amended ? [2] : [1, 2];
  for (const irVersion of versions) {
    const selected = versioned(input, irVersion);
    const oracle =
      irVersion === 1 &&
      currentBehaviorCases.some((entry) => entry.name === input.name)
        ? "current"
        : "TLC";
    test(
      `operator sharing v${irVersion} TS/${oracle}: ${input.name}`,
      {
        timeout: 60_000,
      },
      () => {
        assertVersionedOracle(
          fixture,
          irVersion,
          capture(() => ({
            kind: "safety",
            result: checkSafety(
              core,
              compileTlaSafety(selected),
              budget(input.name),
            ),
          })),
        );
      },
    );
    test(
      `operator sharing v${irVersion} Rust/${oracle}: ${input.name}`,
      {
        timeout: 60_000,
      },
      () => {
        assertVersionedOracle(
          fixture,
          irVersion,
          capture(() => ({
            kind: "safety",
            result: checkTlaSource(core, selected, budget(input.name)),
          })),
        );
      },
    );
  }
  const selected = versioned(input, 2);
  test(`operator sharing v2 frontend parity: ${input.name}`, () => {
    const ts = capture(() => ({
      kind: "compiled",
      program: compileTlaSafety(selected),
    }));
    const rust = capture(() => ({
      kind: "compiled",
      program: compileTlaSource(core, selected),
    }));
    assert.deepEqual(normalize(ts), normalize(rust));
    assert.equal(ts.kind, "compiled", JSON.stringify(ts));
    assert.equal(rust.kind, "compiled", JSON.stringify(rust));
  });
  test(
    `operator sharing v2 native/Wasm compile parity: ${input.name}`,
    {
      timeout: 60_000,
    },
    () => {
      const request = { kind: "compile-tla" as const, ...selected };
      const wasm = capture(() => core.request({ version: 1, request }));
      const response = native(request);
      assert.deepEqual(response, wasm);
      assert.ok(isRecord(response));
      assert.equal(response.kind, "compiled", JSON.stringify(response));
    },
  );
  test(
    `operator sharing v2 native/Wasm check parity: ${input.name}`,
    {
      timeout: 60_000,
    },
    () => {
      const request = {
        kind: "check-tla" as const,
        ...selected,
        bounds: budget(input.name),
      };
      const wasm = capture(() => core.request({ version: 1, request }));
      const response = native(request);
      assert.deepEqual(response, wasm);
      assertV2Oracle(fixture, response);
    },
  );
  if (
    amended ||
    currentBehaviorCases.some((entry) => entry.name === input.name)
  ) {
    test(
      `operator sharing v2 TS native/Wasm check parity: ${input.name}`,
      {
        timeout: 60_000,
      },
      () => {
        const program = compileTlaSafety(selected);
        const request = {
          kind: "check-safety" as const,
          program,
          bounds: budget(input.name),
        };
        const wasm = capture(() => core.request({ version: 1, request }));
        const response = native(request);
        assert.deepEqual(response, wasm);
        assertV2Oracle(fixture, response);
      },
    );
    for (const frontend of amended ? [] : (["TS", "Rust"] as const))
      test(
        `operator sharing v1 ${frontend} current native/Wasm result: ${input.name}`,
        {
          timeout: 60_000,
        },
        () => {
          if (frontend === "TS") {
            const compiled = capture(() =>
              compileTlaSafety(versioned(input, 1)),
            );
            if ("kind" in compiled) {
              assertVersionedOracle(fixture, 1, compiled);
              return;
            }
            const request = {
              kind: "check-safety" as const,
              program: compiled,
              bounds: budget(input.name),
            };
            const wasm = capture(() => core.request({ version: 1, request }));
            const response = native(request);
            assert.deepEqual(response, wasm);
            assertVersionedOracle(fixture, 1, response);
          } else {
            const request = {
              kind: "check-tla" as const,
              ...versioned(input, 1),
              bounds: budget(input.name),
            };
            const wasm = capture(() => core.request({ version: 1, request }));
            const response = native(request);
            assert.deepEqual(response, wasm);
            assertVersionedOracle(fixture, 1, response);
          }
        },
      );
  }
}

const semanticOutcome = (outcome: unknown): unknown => {
  if (!isRecord(outcome) || !isRecord(outcome.result)) return outcome;
  return {
    ...outcome,
    result: Object.fromEntries(
      Object.entries(outcome.result).filter(([key]) => key !== "work"),
    ),
  };
};
const existingBounds = {
  maxDepth: 6,
  maxStates: 1_000,
  maxWork: 100_000,
  maxTemporalNodes: 10_000,
};
for (const input of existing)
  test(
    `operator sharing existing cohort v2 acceptance and v1 semantics: ${input.name}`,
    {
      timeout: 60_000,
    },
    () => {
      const selected = versioned(input, 2);
      const request = { kind: "compile-tla" as const, ...selected };
      const ts = capture(() => ({
        kind: "compiled" as const,
        program: compileTlaSafety(selected),
      }));
      const rust = capture(() => core.request({ version: 1, request }));
      assert.deepEqual(native(request), rust);
      if (
        ["string-len-supplementary", "string-tail-prefix"].includes(input.name)
      ) {
        assert.deepEqual(ts, {
          kind: "error",
          code: "protocol",
          message:
            "Unsupported TLA input: TLC 1.7.4 string literal contains a character outside the byte range",
        });
        assert.deepEqual(rust, {
          kind: "error",
          code: "protocol",
          message:
            "Unsupported TLA input: TLC 1.7.4 string literal outside byte range",
        });
        return;
      }
      assert.deepEqual(normalize(ts), normalize(rust));
      assert.equal(ts.kind, "compiled", JSON.stringify(ts));
      assert.equal(rust.kind, "compiled", JSON.stringify(rust));
      assert.ok(ts.kind === "compiled");
      const check = {
        kind: "check-tla" as const,
        ...selected,
        bounds: existingBounds,
      };
      const wasm = capture(() => core.request({ version: 1, request: check }));
      assert.deepEqual(native(check), wasm);
      const tsCheck = {
        kind: "check-safety" as const,
        program: ts.program,
        bounds: existingBounds,
      };
      const tsWasm = capture(() =>
        core.request({ version: 1, request: tsCheck }),
      );
      assert.deepEqual(native(tsCheck), tsWasm);
      assert.deepEqual(tsWasm, wasm);
      const v1 = capture(() =>
        core.request({
          version: 1,
          request: { ...check, irVersion: 1 },
        }),
      );
      const tsV1 = capture(() => ({
        kind: "safety" as const,
        result: checkSafety(
          core,
          compileTlaSafety(versioned(input, 1)),
          existingBounds,
        ),
      }));
      assert.equal(wasm.kind, v1.kind, JSON.stringify({ v1, v2: wasm }));
      assert.deepEqual(semanticOutcome(tsV1), semanticOutcome(v1));
      assert.deepEqual(semanticOutcome(wasm), semanticOutcome(v1));
    },
  );

test("operator sharing explicit v2 compiles on both frontends and bridges", () => {
  const selected = versioned(inputAt("sharing-unused-argument"), 2);
  const ts = compileTlaSafety(selected);
  const rust = compileTlaSource(core, selected);
  assert.equal(ts.version, 2);
  assert.equal(rust.version, 2);
  assert.deepEqual(normalize(ts), normalize(rust));
  for (const kind of ["compile-tla", "check-tla"] as const) {
    const request =
      kind === "compile-tla"
        ? { kind, ...selected }
        : { kind, ...selected, bounds };
    const wasm = capture(() => core.request({ version: 1, request }));
    assert.deepEqual(native(request), wasm);
    assert.equal(wasm.kind, kind === "compile-tla" ? "compiled" : "safety");
  }
});

test("operator sharing v2 serialized chain grows linearly", () => {
  const sizes = [10, 20, 30, 60].map((depth) => {
    const input = inputAt(`sharing-chain-${depth}`);
    const result = capture(() => ({
      kind: "compiled",
      program: compileTlaSafety(input),
    }));
    assert.equal(result.kind, "compiled", JSON.stringify(result));
    assert.ok("program" in result);
    assert.equal(result.program.version, 2);
    return JSON.stringify(result.program).length;
  });
  const [a, b, c, d] = sizes;
  assert.ok(
    a !== undefined && b !== undefined && c !== undefined && d !== undefined,
  );
  assert.ok(a < b && b < c && c < d);
  assert.ok(d <= a * 6);
  assert.ok(d - c <= (c - b) * 3.2);
});

test("operator sharing v1 unused argument is green and an eager-argument fault is detected", () => {
  const input = versioned(inputAt("sharing-unused-argument"), 1);
  const fixture = golden.cases.find(
    (entry) => entry.name === "sharing-unused-argument",
  );
  assert.ok(fixture);
  const eager = {
    ...input,
    source: input.source.replace("Ignore(p) == TRUE", "Ignore(p) == p = p"),
  };
  for (const compile of [
    (value: TlaSourceOptions) => compileTlaSafety(value),
    (value: TlaSourceOptions) => compileTlaSource(core, value),
  ]) {
    assertOracle(
      fixture,
      capture(() => ({
        kind: "safety",
        result: checkSafety(core, compile(input), bounds),
      })),
    );
    const faulty = capture(() => ({
      kind: "safety",
      result: checkSafety(core, compile(eager), bounds),
    }));
    assert.equal(faulty.kind, "error");
    assert.throws(() => assertOracle(fixture, faulty), assert.AssertionError);
  }
});

const freezePhase = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(freezePhase);
  if (!isRecord(value)) return value;
  if (value.op === "or") return { ...value, left: freezePhase(value.left) };
  if (value.op === "prime") return freezePhase(value.argument);
  return Object.fromEntries(
    Object.entries(value).map(([key, entry]) => [
      key,
      value.op === "read" && key === "next" ? false : freezePhase(entry),
    ]),
  );
};
for (const name of ["sharing-prime", "sharing-enabled"])
  test(`operator sharing v1 detects state-fixed phase fault: ${name}`, () => {
    const fixture = golden.cases.find((entry) => entry.name === name);
    assert.ok(fixture);
    const input = versioned(inputAt(name), 1);
    for (const program of [
      compileTlaSafety(input),
      compileTlaSource(core, input),
    ]) {
      assertOracle(fixture, {
        kind: "safety",
        result: checkSafety(core, program, bounds),
      });
      const faulty = capture(() => ({
        kind: "safety",
        result: checkSafety(
          core,
          freezePhase(program) as SafetyProgram,
          bounds,
        ),
      }));
      assert.throws(() => assertOracle(fixture, faulty), assert.AssertionError);
    }
  });

for (const [name, definitions, safe] of [
  ["self-cycle", "RECURSIVE F(_)\nF(p) == F(p)", "F(TRUE)"],
  ["mutual-cycle", "F(p) == G(p)\nG(p) == F(p)", "F(TRUE)"],
  ["arity", "Id(p) == p", "Id(TRUE, FALSE)"],
] as const)
  for (const irVersion of [1, 2] as const)
    test(`operator sharing v${irVersion} source refusal: ${name}`, () => {
      const input = {
        source: `---- MODULE Refusal ----\nVARIABLE x\n${definitions}\nInit == x = 0\nNext == UNCHANGED x\nSafe == ${safe}\n====`,
        config: "INIT Init\nNEXT Next\nINVARIANT Safe",
        irVersion,
      };
      assert.throws(() => compileTlaSafety(input), PortableCoreError);
      if (name === "self-cycle" && irVersion === 2) {
        const program = compileTlaSource(core, input);
        const result = checkSafety(core, program, bounds);
        assert.equal(result.status, "bounded");
        assert.equal(result.reason, "evaluation");
        assert.ok(result.work > 0);
        assert.deepEqual(native({ kind: "check-tla", ...input, bounds }), {
          kind: "safety",
          result,
        });
      } else
        assert.throws(() => compileTlaSource(core, input), PortableCoreError);
    });

const truth: SafetyExpressionV2 = {
  op: "literal",
  value: { type: "bool", value: true },
};
const parameter: SafetyExpressionV2 = { op: "parameter", slot: 0 };
const call = (
  definition: number,
  arguments_: readonly SafetyExpressionV2[] = [truth],
  captures: readonly SafetyExpressionV2[] = [],
): SafetyExpressionV2 => ({
  op: "call",
  definition,
  arguments: arguments_,
  captures,
});
const definition = (
  id: number,
  body: SafetyExpressionV2 = parameter,
): SafetyDefinition => ({
  id,
  parameterCount: 1,
  captureCount: 0,
  bodyKind: "expression",
  localScope: 0,
  body,
});
const minimalShared = (): SafetyProgramV2 => ({
  format: "modelt.safety-program",
  version: 2,
  name: "Shared",
  variables: ["x"],
  initial: [
    {
      name: "Init",
      steps: [
        {
          kind: "assign",
          slot: 0,
          value: { op: "literal", value: { type: "int", value: "0" } },
        },
      ],
    },
  ],
  next: [
    {
      name: "Next",
      steps: [
        {
          kind: "assign",
          slot: 0,
          value: { op: "read", slot: 0, next: false },
        },
      ],
    },
  ],
  invariants: [{ name: "Safe", expression: call(0) }],
  assumptions: [],
  checkDeadlock: false,
  definitions: [definition(0)],
});
const sharedResponse = (program: SafetyProgram, maxWork = bounds.maxWork) => {
  const request = {
    kind: "check-safety" as const,
    program,
    bounds: { ...bounds, maxWork },
  };
  const wasm = capture(() => core.request({ version: 1, request }));
  assert.deepEqual(native(request), wasm);
  return wasm;
};
const assertSharedAccepted = (program: SafetyProgram) => {
  const response = sharedResponse(program);
  assert.equal(response.kind, "safety", JSON.stringify(response));
  assert.ok("result" in response);
  assert.equal(response.result.status, "complete");
  assert.equal(response.result.states, 1);
  assert.equal(response.result.generated, 2);
  return response.result;
};

test("operator sharing v2 minimal valid IR acceptance control", () => {
  assertSharedAccepted(minimalShared());
});

for (const [name, invalid] of [
  [
    "self-cycle",
    () => ({
      ...minimalShared(),
      definitions: [definition(0, call(0, [parameter]))],
    }),
  ],
  [
    "mutual-cycle",
    () => ({
      ...minimalShared(),
      definitions: [
        definition(0, call(1, [parameter])),
        definition(1, call(0, [parameter])),
      ],
    }),
  ],
  [
    "definition ID",
    () => ({
      ...minimalShared(),
      invariants: [{ name: "Safe", expression: call(1) }],
    }),
  ],
  [
    "arity",
    () => ({
      ...minimalShared(),
      invariants: [{ name: "Safe", expression: call(0, []) }],
    }),
  ],
  [
    "capture",
    () => ({
      ...minimalShared(),
      invariants: [{ name: "Safe", expression: call(0, [truth], [truth]) }],
    }),
  ],
  [
    "bound slot",
    () => ({
      ...minimalShared(),
      definitions: [definition(0, { op: "bound", slot: 0 })],
    }),
  ],
  [
    "phase",
    () => ({
      ...minimalShared(),
      definitions: [definition(0, { op: "read", slot: 0, next: true })],
    }),
  ],
] as const) {
  test(`operator sharing v2 IR ${name} refusal paired with valid control`, () => {
    assertSharedAccepted(minimalShared());
    const response = sharedResponse(invalid());
    assert.equal(response.kind, "error", JSON.stringify(response));
    assert.ok("code" in response);
    assert.equal(response.code, "protocol");
    assert.ok("message" in response);
    assert.doesNotMatch(response.message, /version|not implemented/);
  });
}

const replaceCaptures = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(replaceCaptures);
  if (!isRecord(value)) return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, entry]) => [
      key,
      value.op === "call" && key === "captures" && Array.isArray(entry)
        ? entry.map(() => ({
            op: "literal",
            value: { type: "int", value: "9" },
          }))
        : replaceCaptures(entry),
    ]),
  );
};
test("operator sharing v2 same-name lexical capture fault is detected", () => {
  const input = versioned(inputAt("sharing-lexical-capture"), 2);
  for (const program of [
    compileTlaSafety(input),
    compileTlaSource(core, input),
  ]) {
    assertSharedAccepted(program);
    assert.equal(program.version, 2);
    assert.ok("definitions" in program);
    assert.ok(program.definitions.some((entry) => entry.captureCount > 0));
    const response = sharedResponse(replaceCaptures(program) as SafetyProgram);
    assert.deepEqual(response, {
      kind: "error",
      code: "protocol",
      message: "Constant invariant or property is false",
    });
  }
});

for (const name of ["sharing-prime", "sharing-enabled"])
  test(`operator sharing v2 detects phase fault: ${name}`, () => {
    const fixture = golden.cases.find((entry) => entry.name === name);
    assert.ok(fixture);
    const input = versioned(inputAt(name), 2);
    for (const program of [
      compileTlaSafety(input),
      compileTlaSource(core, input),
    ]) {
      assertOracle(fixture, sharedResponse(program));
      const faulty = sharedResponse(freezePhase(program) as SafetyProgram);
      assert.throws(() => assertOracle(fixture, faulty), assert.AssertionError);
    }
  });

test("operator sharing v2 public nested identity charges four work units", () => {
  const shared = {
    ...minimalShared(),
    invariants: [],
    assumptions: [{ name: "Safe", expression: call(0, [call(0)]) }],
  };
  const plain = {
    ...shared,
    assumptions: [{ name: "Safe", expression: truth }],
  };
  const baseline = assertSharedAccepted(plain);
  const charged = assertSharedAccepted(shared);
  assert.equal(charged.work - baseline.work, 4);
  for (const delta of [3, 4, 5]) {
    const response = sharedResponse(shared, baseline.work + delta);
    assert.equal(response.kind, "safety", JSON.stringify(response));
    assert.ok("result" in response);
    if (delta === 3) {
      assert.equal(response.result.status, "bounded");
      assert.equal(response.result.reason, "work");
      assert.equal(response.result.work, baseline.work + 3);
    } else {
      assert.equal(response.result.status, "complete");
      assert.equal(response.result.work, baseline.work + 4);
    }
  }
});

const callEdges = (value: unknown): number => {
  if (Array.isArray(value))
    return value.reduce((sum, entry) => sum + callEdges(entry), 0);
  if (!isRecord(value)) return 0;
  return (
    (value.op === "call" ? 1 : 0) +
    Object.values(value).reduce<number>(
      (sum, entry) => sum + callEdges(entry),
      0,
    )
  );
};
test("operator sharing v2 definition and call-edge counts grow linearly", () => {
  for (const compile of [
    (value: TlaSourceOptions) => compileTlaSafety(value),
    (value: TlaSourceOptions) => compileTlaSource(core, value),
  ]) {
    const counts = [10, 20, 30, 60].map((depth) => {
      const program = compile(versioned(inputAt(`sharing-chain-${depth}`), 2));
      assert.equal(program.version, 2);
      assert.ok("definitions" in program);
      assert.ok(
        program.definitions.every((entry, index) => entry.id === index),
      );
      assert.ok(
        program.definitions.every((entry) => entry.bodyKind === "expression"),
      );
      return {
        depth,
        definitions: program.definitions.length,
        edges: callEdges(program),
      };
    });
    for (let index = 1; index < counts.length; index++) {
      const current = counts[index];
      const previous = counts[index - 1];
      assert.ok(current && previous);
      const added = current.depth - previous.depth;
      assert.equal(current.definitions - previous.definitions, added);
      assert.equal(current.edges - previous.edges, added * 2);
    }
  }
});

const sourceControl = (
  declarations: string,
  safe: string,
  next = "UNCHANGED x",
): TlaSourceOptions => ({
  source: `---- MODULE SharingControl ----\nVARIABLE x\n${declarations}\nInit == x = 0\nNext == ${next}\nSafe == ${safe}\n====`,
  config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
  irVersion: 2,
});
const guardChain = [
  "D0 == TRUE",
  ...Array.from(
    { length: 60 },
    (_, index) =>
      `D${index + 1} == IF TRUE THEN TRUE ELSE (D${index} /\\ D${index})`,
  ),
].join("\n");
for (const [name, input] of [
  [
    "capture survives binder shadowing",
    sourceControl("", "\\A c \\in {0}: LET G == c = 0 IN \\A c \\in {1}: G"),
  ],
  [
    "sibling captures outer binder",
    sourceControl("", "\\A c \\in {0}: LET H == c = 0 G == H IN G"),
  ],
  [
    "transitive siblings retain declaring frame",
    sourceControl(
      "",
      "\\A c \\in {0}: LET H == c = 0 G == H F == G IN \\A c \\in {1}: F",
    ),
  ],
  [
    "sibling parameter captures retain declaring frame",
    sourceControl(
      "",
      "\\A c \\in {0}: LET H(p) == p /\\ (c = 0) G(q) == H(q) IN \\A c \\in {1}: G(TRUE)",
    ),
  ],
  [
    "deep expression guard in Next",
    sourceControl(guardChain, "TRUE", "D60 /\\ UNCHANGED x"),
  ],
  [
    "deep expression guard in inlined action",
    sourceControl(
      `${guardChain}\nGuarded == D60 /\\ UNCHANGED x`,
      "TRUE",
      "Guarded",
    ),
  ],
] as const)
  test(
    `operator sharing v2 handwritten source control: ${name}`,
    {
      timeout: 60_000,
    },
    () => {
      const ts = compileTlaSafety(input);
      const rust = compileTlaSource(core, input);
      assert.equal(ts.version, 2);
      assert.equal(rust.version, 2);
      assert.deepEqual(normalize(ts), normalize(rust));
      assertSharedAccepted(ts);
      assertSharedAccepted(rust);
      const compileRequest = { kind: "compile-tla" as const, ...input };
      const wasm = core.request({ version: 1, request: compileRequest });
      assert.equal(wasm.kind, "compiled");
      assert.deepEqual(native(compileRequest), wasm);
      const checkRequest = {
        kind: "check-tla" as const,
        ...input,
        bounds,
      };
      const checked = core.request({ version: 1, request: checkRequest });
      assert.equal(checked.kind, "safety");
      assert.deepEqual(native(checkRequest), checked);
    },
  );

test("operator sharing v2 handwritten source refusal: nested actuals exceed 40", () => {
  const input = sourceControl(
    "Id(p) == p",
    `${"Id(".repeat(41)}TRUE${")".repeat(41)}`,
  );
  const expected = {
    kind: "error",
    code: "protocol",
    message: "Unsupported TLA input: expression or operator nesting exceeds 40",
  };
  assert.deepEqual(
    capture(() => compileTlaSafety(input)),
    expected,
  );
  assert.deepEqual(
    capture(() => compileTlaSource(core, input)),
    expected,
  );
  for (const kind of ["compile-tla", "check-tla"] as const) {
    const request =
      kind === "compile-tla" ? { kind, ...input } : { kind, ...input, bounds };
    assert.deepEqual(
      capture(() => core.request({ version: 1, request })),
      expected,
    );
    assert.deepEqual(native(request), expected);
  }
});

const compileParity = (input: TlaSourceOptions): SafetyProgram => {
  const start = performance.now();
  const ts = compileTlaSafety(input);
  assert.ok(
    performance.now() - start < 5_000,
    "TS compilation exceeded 5 seconds",
  );
  const rustStart = performance.now();
  const rust = compileTlaSource(core, input);
  assert.ok(
    performance.now() - rustStart < 5_000,
    "Rust/Wasm compilation exceeded 5 seconds",
  );
  assert.deepEqual(normalize(ts), normalize(rust));
  const request = { kind: "compile-tla" as const, ...input };
  const nativeResponse = native(request);
  const wasmResponse = core.request({ version: 1, request });
  assert.deepEqual(
    input.irVersion === 1 ? normalize(nativeResponse) : nativeResponse,
    input.irVersion === 1 ? normalize(wasmResponse) : wasmResponse,
  );
  return ts;
};

for (const position of ["invariant", "guard", "prime", "membership"] as const)
  test(
    `operator sharing compile-only forward chain 5000: ${position}`,
    {
      timeout: 60_000,
    },
    () => {
      const terminal =
        position === "membership"
          ? "0..1"
          : position === "prime"
            ? "x"
            : "x = 0";
      const declarations = Array.from(
        { length: 5000 },
        (_, id) => `O${id} == ${id === 4999 ? terminal : `O${id + 1}`}`,
      ).join("\n");
      const input = {
        source: `---- MODULE ForwardChain ----\nEXTENDS Naturals\nVARIABLE x\n${declarations}\nInit == x = 0\nNext == ${position === "guard" ? "O0 /\\ " : ""}UNCHANGED x\nSafe == ${position === "invariant" ? "O0" : position === "prime" ? "O0 = 0" : position === "membership" ? "x \\in O0" : "x = 0"}\nP == [][O0' = x]_x\n====`,
        config: `INIT Init\nNEXT Next\nINVARIANT Safe${position === "prime" ? "\nPROPERTY P" : ""}`,
        irVersion: 2 as const,
      };
      const program = compileParity(input);
      assert.equal(program.version, 2);
      assert.ok(program.definitions && program.definitions.length >= 5000);
    },
  );

test(
  "operator sharing compile-only 5000-definition cycle is a typed refusal",
  {
    timeout: 60_000,
  },
  () => {
    const input = {
      source: `---- MODULE LongCycle ----\nVARIABLE x\n${Array.from({ length: 5000 }, (_, id) => `O${id} == O${(id + 1) % 5000}`).join("\n")}\nInit == x = 0\nNext == UNCHANGED x\nSafe == O0\n====`,
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
      irVersion: 2 as const,
    };
    const ts = capture(() => compileTlaSafety(input));
    const rust = capture(() => compileTlaSource(core, input));
    assert.deepEqual(ts, {
      kind: "error",
      code: "protocol",
      message: "Unsupported TLA input: recursive operator definitions",
    });
    assert.deepEqual(rust, {
      kind: "error",
      code: "protocol",
      message:
        "Unsupported TLA input: recursive operator definitions require RECURSIVE declarations",
    });
    assert.deepEqual(native({ kind: "compile-tla", ...input }), rust);
  },
);

test(
  "operator sharing compile-only sibling LET scopes share declaration storage",
  {
    timeout: 60_000,
  },
  () => {
    const tuple = `<<${Array.from({ length: 64 }, (_, index) => index).join(",")}>>`;
    const input = {
      source: `---- MODULE ScopeCost ----\nVARIABLE x\n${Array.from({ length: 1000 }, (_, id) => `D${id} == ${tuple}`).join("\n")}\nInit == x = 0\nNext == UNCHANGED x\nSafe == <<${Array.from({ length: 1000 }, (_, id) => `LET Local == D${id} IN Local`).join(",")}>>\n====`,
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
      irVersion: 2 as const,
    };
    const program = compileParity(input);
    assert.equal(program.version, 2);
    assert.equal(program.definitions?.length, 2001);
  },
);

for (const arms of [1, 48, 127, 128, 129, 169])
  test(`operator sharing CASE preserves existing limits: ${arms} arms`, () => {
    const source = `---- MODULE CaseDepth ----\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nSafe == CASE ${Array.from({ length: arms }, () => "TRUE -> TRUE").join(" [] ")} [] OTHER -> FALSE\n====`;
    const config = "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE";
    const v1 = compileTlaSafety({ source, config, irVersion: 1 });
    const v1Check = capture(() =>
      checkSafety(core, v1, { maxDepth: 1, maxStates: 2, maxWork: 256 }),
    );
    const input = { source, config, irVersion: 2 as const };
    const ts = capture(() => compileTlaSafety(input));
    const rust = capture(() => compileTlaSource(core, input));
    if (arms > 128) {
      const refusal = {
        kind: "error",
        code: "protocol",
        message:
          "Unsupported TLA input: compiled expression nesting exceeds 128",
      };
      assert.deepEqual(ts, refusal);
      assert.deepEqual(rust, refusal);
      assert.deepEqual(native({ kind: "compile-tla", ...input }), refusal);
      assert.ok(isRecord(v1Check));
      assert.equal(v1Check.kind, "error");
      assert.equal(v1Check.code, "protocol");
      assert.match(String(v1Check.message), /recursion limit exceeded/);
    } else {
      assert.deepEqual(normalize(ts), normalize(rust));
      const program = compileTlaSafety(input);
      const v2Check = capture(() =>
        checkSafety(core, program, { maxDepth: 1, maxStates: 2, maxWork: 256 }),
      );
      if (arms >= 127) {
        assert.ok(isRecord(v1Check) && isRecord(v2Check));
        assert.equal(v1Check.kind, "error");
        assert.equal(v2Check.kind, "error");
        assert.match(String(v1Check.message), /recursion limit exceeded/);
        assert.match(String(v2Check.message), /recursion limit exceeded/);
      }
    }
  });

for (const [label, declarations, next, safe, config] of [
  ["variable alias", "Var == x", "UNCHANGED Var", "x = 0", ""],
  [
    "tuple argument",
    "Id(p) == p\nKeep(p) == UNCHANGED p\nVars == Id(<<x>>)",
    "Keep(Vars)",
    "x = 0",
    "",
  ],
  [
    "lexical capture",
    "",
    "LET Vars == <<x>> Keep == UNCHANGED Vars IN Keep",
    "x = 0",
    "",
  ],
  [
    "ENABLED",
    "Vars == <<x>>\nKeep(p) == UNCHANGED p",
    "Keep(Vars)",
    "ENABLED Keep(Vars)",
    "",
  ],
  [
    "ENABLED only",
    "Vars == <<x>>\nKeep(p) == UNCHANGED p",
    "UNCHANGED x",
    "ENABLED Keep(Vars)",
    "",
  ],
  [
    "ENABLED formal",
    "Vars == <<x>>\nCan(p) == ENABLED (UNCHANGED p)",
    "UNCHANGED x",
    "Can(Vars)",
    "",
  ],
  [
    "ENABLED transitive formal",
    "Vars == <<x>>\nKeep(p) == UNCHANGED p\nCan(p) == ENABLED Keep(p)\nWrap(q) == Can(q)",
    "UNCHANGED x",
    "Wrap(Vars)",
    "",
  ],
  [
    "ENABLED captured formal",
    "Vars == <<x>>\nOuter(p) == LET Can == ENABLED (UNCHANGED p) IN Can",
    "UNCHANGED x",
    "Outer(Vars)",
    "",
  ],
  [
    "ENABLED local formal",
    "Vars == <<x>>",
    "UNCHANGED x",
    "LET Can(p) == ENABLED (UNCHANGED p) IN Can(Vars)",
    "",
  ],
  ["prime", "Var == x", "UNCHANGED x", "x = 0", "\nPROPERTY P"],
] as const)
  test(`operator sharing target alias: ${label}`, () => {
    const input = {
      source: `---- MODULE TargetAlias ----\nVARIABLE x\n${declarations}\nInit == x = 0\nNext == ${next}\nSafe == ${safe}\nP == [][Var' = x]_x\n====`,
      config: `INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE${config}`,
      irVersion: 2 as const,
    };
    const program = compileParity(input);
    const result = checkSafety(core, program, bounds);
    assert.equal(result.status, "complete");
    assert.equal(result.states, 1);
  });

test("operator sharing SPECIFICATION subscript resolves variable aliases", () => {
  const input = {
    source:
      "---- MODULE SubscriptAlias ----\nVARIABLE x\nInner == <<x>>\nVars == Inner\nInit == x = 0\nNext == UNCHANGED Vars\nSpec == Init /\\ [][Next]_Vars\nSafe == x = 0\n====",
    config: "SPECIFICATION Spec\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    irVersion: 2 as const,
  };
  const result = checkSafety(core, compileParity(input), bounds);
  assert.equal(result.status, "complete");
  assert.equal(result.states, 1);
});

test("operator sharing fold retains its frame around ENABLED and a binder", () => {
  const source = String.raw`---- MODULE FoldTarget ----
EXTENDS Naturals, Sequences, Apalache
VARIABLE x
Add(a,b) == IF (\A z \in {9}: a = 0) /\ ENABLED (UNCHANGED x) THEN a + b ELSE 100
Init == x = 0
Next == UNCHANGED x
Safe == ApaFoldSeqLeft(Add, 0, <<0,1>>) = 1
====`;
  const config = "INIT Init\nNEXT Next\nINVARIANT Safe";
  for (const irVersion of [1, 2] as const) {
    const program = compileParity({ source, config, irVersion });
    const result = checkSafety(core, program, bounds);
    assert.equal(result.status, "complete");
    assert.equal(result.states, 1);
    assert.equal(result.generated, 2);
    assert.equal(result.violation, null);
  }
});

test("operator sharing generative signatures preserve target shape and ordinary parameters", () => {
  const input: TlaSourceOptions = {
    source: String.raw`---- MODULE TargetSignatures ----
VARIABLES x, y
Can(v) == IF ENABLED (UNCHANGED v) THEN v = v ELSE FALSE
Init == x = 0 /\ y = 1
Next == UNCHANGED <<x,y>>
Safe == Can(x) /\ Can(y) /\ Can(<<x,y>>) /\ Can(<<y,x>>) /\ Can(<<x,<<y>>>>) /\ Can(x)
====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    irVersion: 2,
  };
  const program = compileParity(input);
  assert.equal(program.version, 2);
  if (program.version !== 2) return;
  const specializations = program.definitions.filter(
    (entry) =>
      entry.bodyKind === "expression" &&
      entry.parameterCount === 1 &&
      entry.body.op === "if",
  );
  assert.equal(specializations.length, 5);
  for (const entry of specializations) {
    assert.ok(JSON.stringify(entry.body).includes('"op":"parameter"'));
    assert.equal(entry.captureCount, 0);
  }
  assertSharedAccepted(program);
});

test("operator sharing generative signatures follow LET captured targets", () => {
  const input: TlaSourceOptions = {
    source: String.raw`---- MODULE CapturedTargets ----
VARIABLES x, y
Outer(v) == LET Alias == v
                Can == IF ENABLED (UNCHANGED Alias) THEN v = v ELSE FALSE
           IN Can
Init == x = 0 /\ y = 1
Next == UNCHANGED <<x,y>>
Safe == Outer(x) /\ Outer(y) /\ Outer(x)
====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    irVersion: 2,
  };
  assertSharedAccepted(compileParity(input));
});

for (const [label, safe] of [
  ["constant", "Can(0)"],
  ["EXCEPT old", "[<<9>> EXCEPT ![1] = Can(@)][1]"],
  ["bound variable", "\\A b \\in {0}: Can(b)"],
  ["computed expression", "Can(x + 0)"],
] as const)
  test(`operator sharing generative target refusal: ${label}`, () => {
    const input: TlaSourceOptions = {
      source: `---- MODULE InvalidTarget ----\nEXTENDS Naturals\nVARIABLE x\nCan(v) == ENABLED (UNCHANGED v)\nInit == x = 0\nNext == UNCHANGED x\nSafe == ${safe}\n====`,
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
      irVersion: 2,
    };
    const expected = {
      kind: "error",
      code: "protocol",
      message:
        "Unsupported TLA input: generative UNCHANGED requires state variables",
    };
    assert.deepEqual(
      capture(() => compileTlaSafety(input)),
      expected,
    );
    assert.deepEqual(
      capture(() => compileTlaSource(core, input)),
      expected,
    );
    assert.deepEqual(native({ kind: "compile-tla", ...input }), expected);
  });

for (const [label, name] of [
  ["generative action", "sharing-action-frame-generative"],
  ["temporal helper", "sharing-action-frame-temporal"],
] as const)
  test(`operator sharing lexical frame acceptance: ${label}`, () => {
    const input = versioned(inputAt(name), 2);
    const fixture = amendedFrameGolden.cases.find(
      (entry) => entry.name === name,
    );
    assert.ok(fixture);
    const program = compileTlaSafety(input);
    const rust = compileTlaSource(core, input);
    assert.deepEqual(normalize(program), normalize(rust));
    assertV2Oracle(fixture, {
      kind: "safety",
      result: checkSafety(core, program, bounds),
    });
    assertV2Oracle(fixture, {
      kind: "safety",
      result: checkTlaSource(core, input, bounds),
    });
  });

for (const [label, declaration, application] of [
  ["parameterized", String.raw`Step(a) == @ = 9 /\ x' = a`, "Step(x)"],
  ["zero arity", String.raw`Step == @ = 9 /\ x' = x`, "Step"],
] as const)
  test(`operator sharing LET free EXCEPT old semantics: ${label}`, () => {
    const input: TlaSourceOptions = {
      source: String.raw`---- MODULE Differential ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Safe == x = 0 /\
  [<<9>> EXCEPT ![1] =
    LET ${declaration}
    IN [<<4>> EXCEPT ![1] = ENABLED ${application}][1]][1]
====`,
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
      irVersion: 2,
    };
    if (label === "zero arity")
      assert.deepEqual(
        capture(() => compileTlaSafety(input)),
        {
          kind: "error",
          code: "protocol",
          message:
            "Unsupported TLA input: zero-arity LET @ across a nested EXCEPT requires the Rust frontend",
        },
      );
    else {
      const result = checkSafety(core, compileTlaSafety(input), bounds);
      assert.equal(result.status, "invariant-violation");
      assert.equal(result.violation?.name, "Safe");
    }
    const program = compileTlaSource(core, input);
    const result = checkSafety(core, program, bounds);
    assert.equal(
      result.status,
      label === "zero arity" ? "complete" : "invariant-violation",
    );
    assert.equal(result.states, 1);
    assert.equal(result.generated, label === "zero arity" ? 2 : 1);
    assert.deepEqual(native({ kind: "compile-tla", ...input }), {
      kind: "compiled",
      program,
    });
    const request = { kind: "check-tla" as const, ...input, bounds };
    assert.deepEqual(core.request({ version: 1, request }), {
      kind: "safety",
      result,
    });
    assert.deepEqual(native(request), { kind: "safety", result });
  });

for (const form of ["head", "explicit"] as const) {
  const binding =
    form === "head"
      ? String.raw`f[i \in @] == i`
      : String.raw`f == [i \in @ |-> i]`;
  const bodyBinding =
    form === "head"
      ? String.raw`f[i \in {9}] == @`
      : String.raw`f == [i \in {9} |-> @]`;
  for (const [label, expression] of [
    ["used", `LET ${binding} IN f[9] = 9`],
    ["unused", `LET ${binding} IN TRUE`],
    ["nested LET", `LET Id(a) == LET ${binding} IN f[9] IN Id(0) = 9`],
    ["ENABLED", `LET ${binding} IN ENABLED (x' = f[9])`],
    ["body control", `LET ${bodyBinding} IN f[9] = {9}`],
  ] as const)
    test(`operator sharing function definition domain free old semantics: ${form}, ${label}`, () => {
      const input = sourceControl(
        "",
        `x = 0 /\\ [<<{9}>> EXCEPT ![1] = ${expression}][1]`,
      );
      const reviewBounds = { maxDepth: 8, maxStates: 100, maxWork: 10_000 };
      for (const program of [
        compileTlaSafety(input),
        compileTlaSource(core, input),
      ]) {
        const result = checkSafety(core, program, reviewBounds);
        assert.equal(result.status, "complete");
        assert.equal(result.states, 1);
        assert.equal(result.generated, 2);
        assert.deepEqual(
          native({ kind: "check-safety", program, bounds: reviewBounds }),
          {
            kind: "safety",
            result,
          },
        );
      }
      const request = {
        kind: "check-tla" as const,
        ...input,
        bounds: reviewBounds,
      };
      const response = core.request({ version: 1, request });
      assert.deepEqual(native(request), response);
      assert.ok(response.kind === "safety");
      assert.equal(response.result.status, "complete");
      assert.equal(response.result.states, 1);
      assert.equal(response.result.generated, 2);
    });

  for (const [label, domain, formal, element, safe, work] of [
    [
      "target",
      "(IF ENABLED (UNCHANGED v) THEN {0} ELSE {})",
      "v",
      "0",
      "x = 0 /\\ F(x)",
      12,
    ],
    [
      "membership",
      String.raw`{IF x \in p THEN 1 ELSE 2}`,
      "p",
      "1",
      "F(Nat)",
      10,
    ],
  ] as const)
    test(`operator sharing function definition domain metadata: ${form}, ${label}`, () => {
      const definition =
        form === "head"
          ? `f[i \\in ${domain}] == i`
          : `f == [i \\in ${domain} |-> i]`;
      const input: TlaSourceOptions = {
        source: `---- MODULE FunctionDomain ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nF(${formal}) == LET ${definition} IN f[${element}] = ${element}\nSafe == ${safe}\n====`,
        config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
        irVersion: 2,
      };
      const reviewBounds = { maxDepth: 8, maxStates: 100, maxWork: 10_000 };
      const expected = {
        kind: "safety",
        result: {
          status: "complete",
          reason: null,
          states: 1,
          generated: 2,
          depth: 0,
          work,
          violation: null,
        },
      };
      const fixture = additionalGolden.cases.find(
        (entry) => entry.name === "sharing-unchanged-parameter",
      );
      assert.ok(fixture);
      assertOracle(fixture, expected);
      const ts = compileParity(input);
      const explicit = compileTlaSafety({
        ...input,
        source: input.source.replace(
          `F(${formal}) == LET ${definition}`,
          `F(${formal}) == LET f == [i \\in ${domain} |-> i]`,
        ),
      });
      assert.deepEqual(normalize(ts), normalize(explicit));
      for (const program of [ts, compileTlaSource(core, input)]) {
        const request = {
          kind: "check-safety" as const,
          program,
          bounds: reviewBounds,
        };
        const wasm = capture(() => core.request({ version: 1, request }));
        assert.deepEqual(wasm, expected);
        assert.deepEqual(native(request), expected);
        assertOracle(fixture, wasm);
        for (const maxWork of [work - 1, work]) {
          const limited = {
            ...request,
            bounds: { ...reviewBounds, maxWork },
          };
          const result = core.request({ version: 1, request: limited });
          assert.deepEqual(native(limited), result);
          assert.ok(result.kind === "safety");
          if (maxWork === work) assert.deepEqual(result, expected);
          else {
            assert.equal(result.result.status, "bounded");
            assert.equal(result.result.reason, "work");
            assert.equal(result.result.work, maxWork);
            assert.equal(result.result.violation, null);
          }
        }
      }
      const request = {
        kind: "check-tla" as const,
        ...input,
        bounds: reviewBounds,
      };
      assert.deepEqual(
        capture(() => core.request({ version: 1, request })),
        expected,
      );
      assert.deepEqual(native(request), expected);
    });
}

for (const [label, declarations, safe] of [
  ["direct", "", "x \\in Nat"],
  ["formal", "Member(p) == x \\in p", "Member(Nat)"],
  ["alias", "N == Nat\nMember(p) == x \\in p", "Member(N)"],
  ["capture", "Outer(p) == LET Member == x \\in p IN Member", "Outer(Nat)"],
  [
    "generative result",
    "Types(p) == IF ENABLED (UNCHANGED x) THEN p ELSE {1}",
    "x \\in Types(Nat)",
  ],
] as const)
  test(`operator sharing membership purpose retains frame: ${label}`, () => {
    const input: TlaSourceOptions = {
      source: `---- MODULE MembershipFrame ----\nEXTENDS Naturals\nVARIABLE x\n${declarations}\nInit == x = 0\nNext == UNCHANGED x\nSafe == ${safe}\n====`,
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
      irVersion: 2,
    };
    assertSharedAccepted(compileParity(input));
  });

test("operator sharing generative target restores the caller of nested aliases", () => {
  const input = sourceControl(
    "Id(p) == p\nCan(v) == ENABLED (UNCHANGED v)",
    "Can(Id(Id(x)))",
  );
  assertSharedAccepted(compileParity(input));
});

test("operator sharing ENABLED action subscripts specialize variable targets", () => {
  const input: TlaSourceOptions = {
    source: String.raw`---- MODULE EnabledSubscripts ----
VARIABLES x, y
Box(v) == ENABLED [x' = x /\ y' = y]_v
Angle(v) == ENABLED <<x' = x /\ y' = y>>_v
Init == x = 0 /\ y = 1
Next == UNCHANGED <<x,y>>
Safe == Box(x) /\ Box(y) /\ ~Angle(x) /\ ~Angle(y)
====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    irVersion: 2,
  };
  assertSharedAccepted(compileParity(input));
});

test("operator sharing ENABLED action subscripts refuse nonvariable targets", () => {
  for (const action of ["[x' = x]_v", "<<x' = x>>_v"]) {
    const input = sourceControl(`Can(v) == ENABLED ${action}`, "Can(0)");
    const ts = capture(() => compileTlaSafety(input));
    assert.ok(isRecord(ts));
    assert.equal(ts.kind, "error");
    assert.equal(ts.code, "protocol");
    assert.match(String(ts.message), /state variables|subscript/);
    assert.deepEqual(
      capture(() => compileTlaSource(core, input)),
      ts,
    );
    assert.deepEqual(native({ kind: "compile-tla", ...input }), ts);
  }
});

test("operator sharing membership purpose crosses IF CASE LET and binder domains", () => {
  const input: TlaSourceOptions = {
    source: String.raw`---- MODULE MembershipPaths ----
EXTENDS Naturals
VARIABLE x
Outer(p) == LET Member == x \in p
                Types(q) == IF TRUE THEN q ELSE {1}
                Cases(q) == CASE TRUE -> q [] OTHER -> {1}
           IN Member /\ x \in Types(p) /\ x \in Cases(p)
Bounded(p) == (\A p \in (IF x \in p THEN {0} ELSE {1}): TRUE)
Init == x = 0
Next == UNCHANGED x
Safe == Outer(Nat) /\ Bounded(Nat)
====`,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    irVersion: 2,
  };
  assertSharedAccepted(compileParity(input));
});

test("operator sharing action capture target follows its source caller", () => {
  const input = sourceControl(
    "Vars == <<x>>\nKeep(p) == LET Can == ENABLED (UNCHANGED p) IN Can /\\ UNCHANGED p",
    "x = 0",
    "Keep(Vars)",
  );
  assertSharedAccepted(compileParity(input));
});

test("operator sharing target alias cycles keep typed diagnostic parity", () => {
  const input = sourceControl(
    "A == B\nB == A\nCan(v) == ENABLED (UNCHANGED v)",
    "Can(A)",
  );
  const expected = {
    kind: "error",
    code: "protocol",
    message: "Unsupported TLA input: recursive operator definitions",
  };
  assert.deepEqual(
    capture(() => compileTlaSafety(input)),
    expected,
  );
  const rustExpected = {
    ...expected,
    message:
      "Unsupported TLA input: recursive operator definitions require RECURSIVE declarations",
  };
  assert.deepEqual(
    capture(() => compileTlaSource(core, input)),
    rustExpected,
  );
  assert.deepEqual(native({ kind: "compile-tla", ...input }), rustExpected);
});

test("operator sharing unused action actuals still require valid syntax", () => {
  const input = sourceControl("Act(p) == UNCHANGED x", "x = 0", "Act(Unknown)");
  const expected = {
    kind: "error",
    code: "protocol",
    message: "Unsupported TLA input: operator Unknown",
  };
  assert.deepEqual(
    capture(() => compileTlaSafety(input)),
    expected,
  );
  assert.deepEqual(
    capture(() => compileTlaSource(core, input)),
    expected,
  );
  assert.deepEqual(native({ kind: "compile-tla", ...input }), expected);
});

for (const [left, right, expected] of [
  ["x = 0", "x = 0", true],
  ["x = 0", "x = 1", false],
  ["x = 1", "x = 0", false],
  ["x = 1", "x = 1", true],
] as const)
  test(`operator sharing v2 handwritten equivalence truth table: ${left}, ${right}`, () => {
    const input = sourceControl("", `(${left}) <=> (${right})`);
    const program = compileParity(input);
    const response = sharedResponse(program);
    const request = { kind: "check-tla" as const, ...input, bounds };
    const rust = capture(() => core.request({ version: 1, request }));
    assert.deepEqual(native(request), rust);
    assert.deepEqual(response, rust);
    assert.equal(response.kind, "safety", JSON.stringify(response));
    assert.ok("result" in response);
    assert.equal(
      response.result.status,
      expected ? "complete" : "invariant-violation",
    );
    assert.equal(response.result.reason, null);
    assert.equal(response.result.states, 1);
    assert.equal(response.result.generated, expected ? 2 : 1);
    assert.deepEqual(
      response.result.violation,
      expected
        ? null
        : {
            name: "Safe",
            trace: [{ action: null, values: [{ type: "int", value: "0" }] }],
          },
    );
  });

for (const [left, right] of [
  ["x", "TRUE"],
  ["x", "FALSE"],
  ["TRUE", "x"],
  ["FALSE", "x"],
] as const)
  test(`operator sharing v2 handwritten equivalence requires boolean operands: ${left}, ${right}`, () => {
    const input = sourceControl("", `(${left}) <=> (${right})`);
    const program = compileParity(input);
    const expected = {
      kind: "error",
      code: "protocol",
      message: "Expected a boolean",
    };
    assert.deepEqual(sharedResponse(program), expected);
    const request = { kind: "check-tla" as const, ...input, bounds };
    assert.deepEqual(
      capture(() => core.request({ version: 1, request })),
      expected,
    );
    assert.deepEqual(native(request), expected);
  });

for (const irVersion of [undefined, 1] as const)
  test(`operator sharing v1 handwritten equivalence remains refused: ${irVersion ?? "default"}`, () => {
    const input = sourceControl("", "(x = 0) <=> (x = 0)");
    const selected = {
      source: input.source,
      config: input.config,
      ...(irVersion === undefined ? {} : { irVersion }),
    };
    const expected = {
      kind: "error",
      code: "protocol",
      message: "Unsupported TLA input: operator <=>",
    };
    assert.deepEqual(
      capture(() => compileTlaSafety(selected)),
      {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Apply <=>",
      },
    );
    assert.deepEqual(
      capture(() => compileTlaSource(core, selected)),
      expected,
    );
    const request = { kind: "compile-tla" as const, ...selected };
    assert.deepEqual(
      capture(() => core.request({ version: 1, request })),
      expected,
    );
    assert.deepEqual(native(request), expected);
  });
