import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { bounds } from "../../evals/experiments/collections-temporal/cases.js";
import { traceInput } from "../../evals/experiments/collections-temporal/tlc.js";
import {
  boundaries,
  foldIndependentControls,
  foldOrderControls,
  foldRejections,
} from "../../evals/experiments/tlc-differential/corpus.js";
import { digest } from "../../evals/experiments/usable/provenance.js";
import type {
  PortableValue,
  SafetyResult,
} from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { decodeSafetyProgram } from "../../packages/idealis/src/safety-program.js";
import { checkSafety } from "../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import { compileTlaSource } from "../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import {
  startCiNative,
  createCiWasm,
  checkCiReference,
} from "../ci/tlc-checkers.js";
import "./operator-sharing-test.js";
import "./operator-sharing-framed-test.js";
import "./operator-sharing-limits-test.js";
import "./operator-sharing-wide-test.js";
import "./operator-sharing-depth-test.js";
import "./operator-sharing-resources-test.js";

interface Golden {
  readonly cases: readonly ({
    name: string;
    sourceSha256: string;
    configSha256: string;
    log: string;
    logSha256: string;
    status: string;
    values?: readonly PortableValue[];
    trace?: readonly (readonly PortableValue[])[];
    states?: number;
    generated?: number;
    unsupported?: string;
  } & (
    | { message: string; location: string }
    | { message?: never; location?: never }
  ))[];
}
const golden = (
  await Promise.all(
    ["tlc-strings", "tlc-case", "tlc-apalache-folds"].map(async (directory) => {
      const fixture: Golden = JSON.parse(
        await readFile(`fixtures/${directory}/expected.json`, "utf8"),
      );
      return fixture.cases.map((entry) => ({ ...entry, directory }));
    }),
  )
).flat();
const bodyErrors = new Map([
  [
    "fold-singleton-error",
    {
      idealis: "Head of an empty sequence",
      tlc: "Attempted to apply Head to the empty sequence.",
    },
  ],
  [
    "fold-independent-overflow",
    {
      idealis: "Safety integer overflow",
      tlc: "Overflow when computing 2147483647+1",
    },
  ],
]);
const core = await createCiWasm();
for (const input of [
  ...boundaries(),
  ...foldOrderControls(),
  ...foldIndependentControls(),
])
  test(`TLC regression: ${input.name}`, async () => {
    if (input.expectedRejection) {
      assert.throws(() => compileTlaSafety(input), /TLC 1.7.4 string literal/);
      return;
    }
    const fixture = golden.find((entry) => entry.name === input.name);
    if (fixture) {
      assert.equal(fixture.sourceSha256, digest(input.source));
      assert.equal(fixture.configSha256, digest(input.config));
      assert.equal(
        fixture.logSha256,
        digest(await readFile(`fixtures/${fixture.directory}/${fixture.log}`)),
      );
    }
    if (input.goldenFrontendParity) {
      assert.ok(fixture, input.name);
      assert.deepEqual(compileTlaSource(core, input), compileTlaSafety(input));
    }
    const reference =
      fixture ??
      (await checkCiReference({
        source: input.source,
        config: input.config,
        livenessCheck: "final",
        version: "1.7.4",
      }));
    let result: SafetyResult | undefined;
    let error: PortableCoreError | undefined;
    try {
      result = checkSafety(core, compileTlaSafety(input), bounds);
    } catch (failure) {
      assert.ok(failure instanceof PortableCoreError);
      error = failure;
    }
    const status =
      result?.status === "complete" ? "success" : (result?.status ?? "error");
    assert.equal(
      status,
      fixture?.unsupported ? "error" : reference.status,
      `${input.name}: ${JSON.stringify(result ?? error)}`,
    );
    const native = startCiNative();
    try {
      const response = await native.request({
        version: 1,
        request: {
          kind: "check-safety",
          program: compileTlaSafety(input),
          bounds,
        },
      });
      assert.ok(isRecord(response));
      assert.deepEqual(
        response,
        result
          ? { kind: "safety", result }
          : { kind: "error", code: error?.code, message: error?.message },
      );
    } finally {
      await native.close();
    }
    if (fixture?.unsupported) {
      assert.ok(error?.message.includes(fixture.unsupported), error?.message);
      return;
    }
    if (fixture?.status === "error") {
      assert.ok(
        error && !error.message.includes("set fold order"),
        `${input.name}: ${error?.message}`,
      );
      const expectedError = bodyErrors.get(input.name);
      if (expectedError) {
        assert.equal(error.message, expectedError.idealis, input.name);
        const log = await readFile(
          `fixtures/${fixture.directory}/${fixture.log}`,
          "utf8",
        );
        assert.ok(log.includes(expectedError.tlc), input.name);
      }
    }
    if (fixture?.values)
      assert.deepEqual(result?.violation?.trace[0]?.values, fixture.values);
    if (fixture?.trace)
      assert.deepEqual(
        result?.violation?.trace.map((state) => state.values),
        fixture.trace,
      );
    if (fixture?.states !== undefined)
      assert.equal(result?.states, fixture.states);
    if (fixture?.generated !== undefined)
      assert.equal(result?.generated, fixture.generated);
    if (fixture?.message !== undefined) {
      assert.ok(fixture.location, `${input.name}: missing error location`);
      assert.ok(error?.message.includes(fixture.message), error?.message);
      assert.ok(error?.message.includes(fixture.location), error?.message);
    }
    if (result?.violation?.trace.length && !fixture) {
      const forced = traceInput(input, compileTlaSafety(input), result);
      const witness = await checkCiReference({
        ...forced,
        version: "1.7.4",
        livenessCheck: "final",
      });
      assert.equal(witness.status, "success", input.name);
      assert.equal(witness.distinctStates, result.violation.trace.length);
    }
  });

const singletonInputs = [
  "fold-unsupported-string-direct",
  "fold-unsupported-string-tuple",
  "fold-unsupported-string-set",
  "fold-unsupported-record-direct",
  "fold-unsupported-record-tuple",
  "fold-unsupported-record-set",
  "fold-set-functions-with-records",
  "fold-set-wide-single-function",
  "fold-set-boolean-function-keys",
];
for (const input of [
  ...foldIndependentControls(),
  ...foldOrderControls().filter((entry) =>
    singletonInputs.includes(entry.name),
  ),
])
  test(`fold independence agrees across frontends, IR versions and checkers: ${input.name}`, async () => {
    const outcome = async (irVersion: 1 | 2) => {
      const versioned = { ...input, irVersion };
      const program = compileTlaSource(core, versioned);
      assert.deepEqual(program, compileTlaSafety(versioned));
      let wasm: unknown;
      try {
        wasm = { kind: "safety", result: checkSafety(core, program, bounds) };
      } catch (failure) {
        assert.ok(failure instanceof PortableCoreError);
        wasm = { kind: "error", code: failure.code, message: failure.message };
      }
      const native = startCiNative();
      try {
        const response = await native.request({
          version: 1,
          request: { kind: "check-safety", program, bounds },
        });
        assert.deepEqual(response, wasm);
      } finally {
        await native.close();
      }
      return wasm;
    };
    const fixture = golden.find((entry) => entry.name === input.name);
    assert.ok(fixture, input.name);
    const first = await outcome(1);
    const second = await outcome(2);
    assert.ok(isRecord(first) && isRecord(second));
    if (first.kind === "error") {
      assert.equal(first.code, "protocol");
      assert.equal(second.kind, "error");
      assert.equal(second.code, first.code);
      assert.equal(second.message, first.message);
      assert.equal(
        fixture.status === "error" || fixture.unsupported !== undefined,
        true,
      );
      return;
    }
    assert.equal(second.kind, "safety");
    assert.ok(isRecord(first.result) && isRecord(second.result));
    assert.equal(first.result.status, second.result.status);
    assert.equal(first.result.states, second.result.states);
    assert.deepEqual(first.result.violation, second.result.violation);
    assert.equal(fixture.unsupported, undefined);
  });

test("unmatched CASE IR requires a nonempty location and exact fields", () => {
  const input = boundaries().find(
    (input) => input.name === "case-expression-no-match",
  );
  assert.ok(input);
  const program = compileTlaSafety(input);
  assert.deepEqual(decodeSafetyProgram(program), program);
  for (const expression of [
    { op: "unmatched-case" },
    { op: "unmatched-case", location: "" },
    { op: "unmatched-case", location: 1 },
    { op: "unmatched-case", location: "line 3, column 13", extra: true },
  ]) {
    const invalid = {
      ...program,
      initial: [
        {
          name: "Init",
          steps: [{ kind: "assign", slot: 0, value: expression }],
        },
      ],
    };
    assert.throws(() => decodeSafetyProgram(invalid), PortableCoreError);
  }
});

for (const input of foldRejections())
  test(`Apalache fold rejects: ${input.name}`, async () => {
    assert.equal(input.goldenFrontendParity, true);
    const fixture = golden.find((entry) => entry.name === input.name);
    assert.ok(fixture, input.name);
    assert.equal(fixture.sourceSha256, digest(input.source));
    assert.equal(fixture.configSha256, digest(input.config));
    assert.equal(
      fixture.logSha256,
      digest(await readFile(`fixtures/${fixture.directory}/${fixture.log}`)),
    );
    assert.throws(() => compileTlaSafety(input));
    if (
      ["fold-reject-lambda", "fold-reject-operator-parameter"].includes(
        input.name,
      )
    ) {
      assert.equal(fixture.status, "invariant-violation");
      const native = startCiNative();
      try {
        for (const irVersion of [1, 2] as const) {
          const program = compileTlaSource(core, { ...input, irVersion });
          const result = checkSafety(core, program, bounds);
          assert.equal(result.status, "invariant-violation");
          assert.equal(result.states, 1);
          assert.equal(result.violation?.name, "Prop");
          assert.deepEqual(result.violation?.trace[0]?.values, [
            { type: "int", value: "1" },
          ]);
          const response = await native.request({
            version: 1,
            request: { kind: "check-safety", program, bounds },
          });
          assert.deepEqual(response, { kind: "safety", result });
        }
      } finally {
        await native.close();
      }
      return;
    }
    assert.throws(() => compileTlaSource(core, input), PortableCoreError);
    const native = startCiNative();
    try {
      const response = await native.request({
        version: 1,
        request: {
          kind: "compile-tla",
          source: input.source,
          config: input.config,
        },
      });
      assert.ok(isRecord(response));
      assert.equal(response.kind, "error");
    } finally {
      await native.close();
    }
  });

test("fold IR requires exact fields, an explicit order and valid slot counts", () => {
  const input = boundaries().find(
    (input) => input.name === "fold-set-integers",
  );
  assert.ok(input);
  const program = compileTlaSafety(input);
  assert.deepEqual(decodeSafetyProgram(program), program);
  const fold = program.initial[0]?.steps[0];
  assert.ok(fold?.kind === "assign" && fold.value.op === "fold");
  for (const invalid of [
    { ...fold.value, order: "right" },
    { ...fold.value, accumulator: -1 },
    { ...fold.value, accumulator: 1.5 },
    { ...fold.value, extra: true },
    { ...fold.value, initial: { op: "bound", slot: -1 } },
    { ...fold.value, body: { op: "unknown" } },
    Object.fromEntries(
      Object.entries(fold.value).filter(([key]) => key !== "collection"),
    ),
  ]) {
    assert.throws(
      () =>
        decodeSafetyProgram({
          ...program,
          initial: [
            {
              name: "Init",
              steps: [{ kind: "assign", slot: 0, value: invalid }],
            },
          ],
        }),
      PortableCoreError,
    );
  }
});

test("fold calls preserve expansion limits and reject unsupported imports and operators", () => {
  for (const input of [
    {
      source:
        "---- MODULE Modules ----\nEXTENDS Apalache, Reals\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\n====",
      config: "INIT Init\nNEXT Next",
    },
    {
      source:
        '---- MODULE UnsupportedVariant ----\nEXTENDS Apalache, Variants\nVARIABLE x\nInit == x = VariantFilter("Tag", {})\nNext == UNCHANGED x\n====',
      config: "INIT Init\nNEXT Next",
    },
    {
      source:
        '---- MODULE UnsupportedTLC ----\nEXTENDS Apalache, TLC\nVARIABLE x\nInit == x = Print("tag", 0)\nNext == UNCHANGED x\n====',
      config: "INIT Init\nNEXT Next",
    },
    {
      source: `---- MODULE DeepFold ----\nEXTENDS Apalache\nVARIABLE x\nOp(a, b) == a\nInit == x = ${"ApaFoldSet(Op, ".repeat(50)}0${", {})".repeat(50)}\nNext == UNCHANGED x\n====`,
      config: "INIT Init\nNEXT Next",
    },
  ]) {
    assert.throws(() => compileTlaSafety(input), /Unsupported TLA input/);
    assert.throws(() => compileTlaSource(core, input), PortableCoreError);
  }
});
