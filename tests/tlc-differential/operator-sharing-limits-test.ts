import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { digest } from "../../evals/experiments/usable/provenance.js";
import type {
  PortableCommand,
  SafetyDefinition,
  SafetyExpressionV2,
  SafetyProgramV2,
} from "../../packages/idealis/src/contracts.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import {
  directory,
  type Limits,
  type Measured,
  mebibytes,
  runNative,
  runTypeScript,
  runWasm,
  wasmPath,
} from "./operator-sharing-support.js";

const bounds = { maxDepth: 8, maxStates: 100, maxWork: 100_000 };
const bounded: Limits = { milliseconds: 5_000, bytes: mebibytes(256) };
const nesting128 = {
  kind: "error",
  code: "protocol",
  message: "Unsupported TLA input: compiled expression nesting exceeds 128",
};
const nesting40 = {
  kind: "error",
  code: "protocol",
  message: "Unsupported TLA input: expression or operator nesting exceeds 40",
};

const flatSource = (operands: number) => ({
  source: `---- MODULE Differential ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nSafe == ${Array.from({ length: operands }, () => "TRUE").join(" /\\ ")}\n====\n`,
  config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
  irVersion: 2 as const,
});
const thrownOutcome = (run: () => unknown) => {
  try {
    return run();
  } catch (error) {
    return error instanceof Error
      ? {
          kind: "thrown",
          name: error.name,
          code: "code" in error ? String(error.code) : "",
          message: error.message,
        }
      : {
          kind: "thrown",
          name: typeof error,
          code: "",
          message: String(error),
        };
  }
};
const protocolOutcome = (outcome: unknown) =>
  isRecord(outcome) && outcome.kind === "thrown" && outcome.code === "protocol"
    ? { kind: "error", code: "protocol", message: outcome.message }
    : outcome;

test("operator sharing b20 v2 flat conjunction 50000 native: refused without aborting", async () => {
  const measured = await runNative(
    { kind: "check-tla", ...flatSource(50_000), bounds },
    bounded,
  );
  assert.deepEqual(measured.outcome, nesting128);
});

test("operator sharing b20 v2 flat conjunction 5000 Wasm: refused with the nesting message", async () => {
  const fresh = await createPortableWasm(await readFile(wasmPath));
  const outcome = protocolOutcome(
    thrownOutcome(() =>
      fresh.request({
        version: 1,
        request: { kind: "compile-tla", ...flatSource(5_000) },
      }),
    ),
  );
  assert.deepEqual(outcome, nesting128);
});

test("operator sharing b20 v2 flat conjunction 5000 TS frontend: refused with the nesting message", () => {
  const outcome = protocolOutcome(
    thrownOutcome(() => compileTlaSafety(flatSource(5_000))),
  );
  assert.deepEqual(outcome, nesting128);
});

const chain = (length: number) =>
  `V0 == x\n${Array.from({ length }, (_, index) => `V${index + 1} == <<V${index}, V${index}>>\n`).join("")}`;
const tupleSource = (form: "unchanged" | "specification", length: number) =>
  form === "unchanged"
    ? {
        source: `---- MODULE Differential ----\nEXTENDS Integers\nVARIABLE x\n${chain(length)}Init == x = 0\nNext == UNCHANGED V${length}\nSafe == x = 0\n====\n`,
        config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
        irVersion: 2 as const,
      }
    : {
        source: `---- MODULE Differential ----\nEXTENDS Integers\nVARIABLE x\n${chain(length)}Init == x = 0\nNext == x' = x\nSpec == Init /\\ [][Next]_V${length}\nSafe == x = 0\n====\n`,
        config: "SPECIFICATION Spec\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
        irVersion: 2 as const,
      };
interface Golden {
  readonly name: string;
  readonly source: string;
  readonly config: string;
  readonly sourceSha256: string;
  readonly configSha256: string;
  readonly states: number;
  readonly generated: number;
}
const golden: { readonly cases: readonly Golden[] } = JSON.parse(
  await readFile(`${directory}/expected-step3f-b20.json`, "utf8"),
);
const fixture = (name: string) => {
  const found = golden.cases.find((entry) => entry.name === name);
  assert.ok(found, name);
  return found;
};

const tupleCases = [
  {
    form: "unchanged",
    length: 6,
    fixture: "sharing-tuple-unchanged-6-step3f-b20",
  },
  {
    form: "unchanged",
    length: 16,
    refusal: /branch exceeds 256 steps|nesting exceeds 40/,
  },
  { form: "unchanged", length: 20, refusal: nesting40 },
  { form: "unchanged", length: 30, refusal: nesting40 },
  {
    form: "specification",
    length: 6,
    fixture: "sharing-tuple-specification-6-step3f-b20",
  },
  {
    form: "specification",
    length: 16,
    fixture: "sharing-tuple-specification-16-step3f-b20",
  },
  { form: "specification", length: 20, refusal: nesting40 },
  { form: "specification", length: 30, refusal: nesting40 },
] as const;

for (const entry of tupleCases)
  if ("fixture" in entry)
    test(`operator sharing b20 tuple source matches the TLC fixture: ${entry.form} ${entry.length}`, async () => {
      const recorded = fixture(entry.fixture);
      const generated = tupleSource(entry.form, entry.length);
      assert.equal(digest(generated.source), recorded.sourceSha256);
      assert.equal(digest(generated.config), recorded.configSha256);
      assert.equal(
        generated.source,
        await readFile(`${directory}/${recorded.source}`, "utf8"),
      );
    });

const assertTupleOutcome = (
  entry: (typeof tupleCases)[number],
  measured: Measured,
) => {
  const outcome = measured.outcome;
  if ("fixture" in entry) {
    const recorded = fixture(entry.fixture);
    assert.ok(isRecord(outcome));
    assert.equal(outcome.kind, "safety", JSON.stringify(outcome));
    assert.ok(isRecord(outcome.result));
    assert.equal(outcome.result.status, "complete");
    assert.equal(outcome.result.states, recorded.states);
    assert.equal(outcome.result.generated, recorded.generated);
    assert.equal(outcome.result.violation, null);
  } else if (entry.refusal instanceof RegExp) {
    assert.ok(isRecord(outcome));
    assert.equal(outcome.kind, "error", JSON.stringify(outcome));
    assert.equal(outcome.code, "protocol");
    assert.match(String(outcome.message), entry.refusal);
  } else assert.deepEqual(outcome, entry.refusal);
  assert.ok(
    measured.elapsedMs < 1_000,
    `took ${Math.round(measured.elapsedMs)} ms`,
  );
  assert.ok(measured.run.peakRssBytes <= mebibytes(256));
};

for (const entry of tupleCases) {
  const input = () => tupleSource(entry.form, entry.length);
  const label = `${entry.form} ${entry.length}`;
  test(`operator sharing b20 v2 tuple ${label} native: bounded in 1 s and 256 MiB`, async () => {
    assertTupleOutcome(
      entry,
      await runNative({ kind: "check-tla", ...input(), bounds }, bounded),
    );
  });
  test(`operator sharing b20 v2 tuple ${label} Wasm: bounded in 1 s and 256 MiB`, async () => {
    assertTupleOutcome(
      entry,
      await runWasm({ kind: "check-tla", ...input(), bounds }, bounded),
    );
  });
  test(`operator sharing b20 v2 tuple ${label} TS frontend: bounded in 1 s and 256 MiB`, async () => {
    assertTupleOutcome(entry, await runTypeScript(input(), bounds, bounded));
  });
}

const forwardingLevels = 3;
const forwardingArguments = 4_096;
const truth: SafetyExpressionV2 = {
  op: "literal",
  value: { type: "bool", value: true },
};
const forwarded = (definition: number): SafetyExpressionV2 => ({
  op: "call",
  definition,
  arguments: Array.from({ length: forwardingArguments }, (_, slot) => ({
    op: "parameter",
    slot,
  })),
  captures: [],
});
const forwardingDefinition = (
  id: number,
  body: SafetyExpressionV2,
  parameterCount = forwardingArguments,
): SafetyDefinition => ({
  id,
  parameterCount,
  captureCount: 0,
  bodyKind: "expression",
  localScope: 0,
  body,
});
const forwardingProgram = (): SafetyProgramV2 => ({
  format: "modelt.safety-program",
  version: 2,
  name: "Forwarding",
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
  invariants: [
    {
      name: "Safe",
      expression: {
        op: "call",
        definition: forwardingLevels,
        arguments: [],
        captures: [],
      },
    },
  ],
  assumptions: [],
  checkDeadlock: false,
  definitions: [
    forwardingDefinition(0, truth),
    ...Array.from({ length: forwardingLevels - 1 }, (_, index) =>
      forwardingDefinition(index + 1, forwarded(index)),
    ),
    forwardingDefinition(
      forwardingLevels,
      {
        op: "call",
        definition: forwardingLevels - 1,
        arguments: Array.from({ length: forwardingArguments }, () => ({
          op: "literal",
          value: { type: "int", value: "0" },
        })),
        captures: [],
      },
      0,
    ),
  ],
});
const forwardingRequest = (): PortableCommand["request"] => ({
  kind: "check-safety",
  program: forwardingProgram(),
  bounds,
});
const assertForwarded = (measured: Measured) => {
  assert.deepEqual(measured.outcome, {
    kind: "safety",
    result: {
      status: "complete",
      reason: null,
      states: 1,
      generated: 2,
      depth: 0,
      work: 12,
      violation: null,
    },
  });
  assert.ok(measured.run.peakRssBytes <= mebibytes(256));
};
const forwardingLimits: Limits = {
  milliseconds: 30_000,
  bytes: mebibytes(256),
};

test("operator sharing b20 v2 forwarding 4096 arguments through 3 levels native: peak RSS within 256 MiB", async () => {
  assertForwarded(await runNative(forwardingRequest(), forwardingLimits));
});

test("operator sharing b20 v2 forwarding 4096 arguments through 3 levels Wasm: peak RSS within 256 MiB", async () => {
  assertForwarded(await runWasm(forwardingRequest(), forwardingLimits));
});
