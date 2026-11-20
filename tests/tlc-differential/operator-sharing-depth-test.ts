import assert from "node:assert/strict";
import { test } from "node:test";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import {
  mebibytes,
  runNative,
  runTypeScript,
  runWasm,
} from "./operator-sharing-support.js";

const flat = (operands: number) =>
  Array.from({ length: operands }, () => "TRUE").join(" /\\ ");
for (const irVersion of [undefined, 1] as const) {
  test(`v1 keeps base acceptance while v2 validates depth: ${String(irVersion)}`, () => {
    const input = (operands: number) => ({
      source: `---- MODULE Depth ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nSafe == ${flat(operands)}\n====`,
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
      ...(irVersion === undefined ? {} : { irVersion }),
    });

    assert.equal(compileTlaSafety(input(8)).version, 1);
    assert.equal(compileTlaSafety(input(130)).version, 1);
    assert.throws(
      () => compileTlaSafety({ ...input(130), irVersion: 2 }),
      /compiled expression nesting exceeds 128/,
    );
  });
}
for (const kind of ["action", "nested-action", "temporal"] as const)
  for (const backend of ["native", "Wasm"] as const)
    test(`anonymous ${kind} checks actual depth before cloning: ${backend}`, async () => {
      const source =
        kind !== "temporal"
          ? `Step(p) == ${kind === "nested-action" ? "Inner(p)\nInner(q) == q" : "p"} /\\ x'=x\nNext == Step(${flat(backend === "native" ? 20000 : 5000)})`
          : `T(p) == []p\nNext == UNCHANGED x\nLive == T(${flat(backend === "native" ? 20000 : 5000)})`;
      const input = {
        source: `---- MODULE Depth ----\nVARIABLE x\nInit == x=0\n${source}\n====`,
        config: `INIT Init\nNEXT Next${kind === "temporal" ? "\nPROPERTY Live" : ""}`,
        irVersion: 2 as const,
      };
      const measured = await (backend === "native" ? runNative : runWasm)(
        {
          kind: "check-tla",
          ...input,
          bounds: { maxDepth: 8, maxStates: 100, maxWork: 100000 },
        },
        { milliseconds: 10000, bytes: mebibytes(256) },
      );
      assert.deepEqual(measured.outcome, {
        kind: "error",
        code: "protocol",
        message:
          "Unsupported TLA input: compiled expression nesting exceeds 128",
      });
    });

for (const kind of ["frame", "direct"] as const)
  for (const operator of ["/\\", "\\/"])
    for (const backend of ["native", "Wasm", "TS"] as const)
      test(`temporal ${kind} ${operator} is bounded before recursive traversal: ${backend}`, async () => {
        for (const operands of [8, backend === "native" ? 20000 : 5000]) {
          const formula = Array.from({ length: operands }, () =>
            kind === "frame" ? "[]p" : "[](x=0)",
          ).join(` ${operator} `);
          const input = {
            source: `---- MODULE Depth ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\n${kind === "frame" ? `T(p) == ${formula}\nLive == T(x=0)` : `Live == ${formula}`}\n====`,
            config: "INIT Init\nNEXT Next\nPROPERTY Live",
            irVersion: 2 as const,
          };
          const bounds = { maxDepth: 8, maxStates: 100, maxWork: 100000 };
          const limits = { milliseconds: 10000, bytes: mebibytes(256) };
          const measured = await (backend === "TS"
            ? runTypeScript(input, bounds, limits)
            : (backend === "native" ? runNative : runWasm)(
                { kind: "check-tla", ...input, bounds },
                limits,
              ));
          if (operands === 8) {
            assert.ok(isRecord(measured.outcome));
            assert.equal(measured.outcome.kind, "safety");
            assert.ok(isRecord(measured.outcome.result));
            assert.equal(measured.outcome.result.status, "complete");
          } else {
            assert.deepEqual(measured.outcome, {
              kind: "error",
              code: "protocol",
              message:
                "Unsupported TLA input: compiled temporal nesting exceeds 128",
            });
          }
        }
      });
