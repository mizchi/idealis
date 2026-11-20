import assert from "node:assert/strict";
import { PortableCoreError } from "../packages/idealis/src/portable-value.js";
import { decodeSafetyProgram } from "../packages/idealis/src/safety-program.js";
import { compileTlaSafety } from "../packages/idealis/src/tla-safety.js";

export const registerSourceContract = (
  test: (name: string, run: () => void) => unknown,
): void => {
  test("compiled symmetry metadata is optional and validates the finite model-value group", () => {
    const program = compileTlaSafety({
      source:
        "---- MODULE Wire ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nSafe == x < 3\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
    });
    const input = { ...program, symmetry: { modelValues: ["a", "b"] } };
    const decoded = decodeSafetyProgram(input);
    assert.deepEqual(decoded, input);
    assert.notEqual(decoded.symmetry, input.symmetry);
    const maximum = {
      ...program,
      symmetry: {
        modelValues: Array.from({ length: 64 }, (_, index) => `n${index}`),
      },
    };
    assert.deepEqual(decodeSafetyProgram(maximum), maximum);
    for (const symmetry of [
      null,
      {},
      { modelValues: "a" },
      { modelValues: ["a"] },
      { modelValues: ["a", "a"] },
      { modelValues: ["n_a", "n_b"] },
      { modelValues: ["a", 1] },
      { modelValues: ["a", "bad name"] },
      { modelValues: ["a", "b\n"] },
      { modelValues: ["a", "b"], unknown: true },
      { modelValues: Array.from({ length: 65 }, (_, index) => `n${index}`) },
    ])
      assert.throws(
        () => decodeSafetyProgram({ ...program, symmetry }),
        PortableCoreError,
      );
  });
  test("compiled IR preserves a snapshot and rejects unsupported wire shapes", () => {
    const program = compileTlaSafety({
      source:
        "---- MODULE Wire ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nSafe == x < 3\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
    });
    const parsed = decodeSafetyProgram(program);
    assert.deepEqual(parsed, program);
    assert.notEqual(parsed, program);
    for (const invalid of [
      { ...program, version: 2 },
      { ...program, extra: true },
      { ...program, variables: ["x", "x"] },
      {
        ...program,
        next: [
          {
            name: "Next",
            steps: [{ kind: "guard", expression: { op: "mystery" } }],
          },
        ],
      },
      {
        ...program,
        invariants: [
          { name: "Safe", expression: { op: "read", slot: 0, next: "true" } },
        ],
      },
      {
        ...program,
        invariants: [
          {
            name: "Safe",
            expression: {
              op: "forall",
              bounds: [{ slot: 0, domain: { kind: "unknown" } }],
              body: { op: "old" },
            },
          },
        ],
      },
      {
        ...program,
        properties: [{ name: "Safe", formula: { kind: "unknown" } }],
      },
    ])
      assert.throws(() => decodeSafetyProgram(invalid), PortableCoreError);
  });
};
