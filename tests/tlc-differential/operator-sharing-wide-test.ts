import assert from "node:assert/strict";
import { test } from "node:test";
import { isRecord } from "../../packages/idealis/src/trace.js";
import {
  mebibytes,
  runNative,
  runTypeScript,
  runWasm,
} from "./operator-sharing-support.js";

const bounds = { maxDepth: 8, maxStates: 100, maxWork: 100_000 };
const limits = { milliseconds: 5_000, bytes: mebibytes(256) };
const input = (form: "empty-target" | "subscript", depth: number) => ({
  source: `---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\nV0 == ${form === "empty-target" ? "<<>>" : "x"}\n${Array.from({ length: depth }, (_, index) => `V${index + 1} == <<${Array.from({ length: 16 }, () => `V${index}`).join(",")}>>`).join("\n")}\nInit == x=0\nNext == ${form === "empty-target" ? `UNCHANGED V${depth} /\\ UNCHANGED x` : "x'=x"}\nSpec == Init /\\ [][Next]_${form === "empty-target" ? "x" : `V${depth}`}\n====`,
  config: "SPECIFICATION Spec\nCHECK_DEADLOCK FALSE",
  irVersion: 2 as const,
});

for (const form of ["empty-target", "subscript"] as const)
  for (const depth of [3, 6])
    for (const backend of ["native", "Wasm", "TS"] as const)
      test(`operator sharing wide ${form} depth ${depth} ${backend}`, async () => {
        const source = input(form, depth);
        const measured =
          backend === "TS"
            ? await runTypeScript(source, bounds, limits)
            : await (backend === "native" ? runNative : runWasm)(
                { kind: "check-tla", ...source, bounds },
                limits,
              );
        if (depth === 6) {
          assert.deepEqual(measured.outcome, {
            kind: "error",
            code: "protocol",
            message:
              "Unsupported TLA input: target expansion exceeds 524288 nodes",
          });
        } else {
          assert.ok(
            isRecord(measured.outcome) && measured.outcome.kind === "safety",
          );
          assert.ok(isRecord(measured.outcome.result));
          assert.equal(measured.outcome.result.status, "complete");
          assert.equal(measured.outcome.result.states, 1);
          assert.equal(measured.outcome.result.generated, 2);
          assert.equal(measured.outcome.result.violation, null);
        }
      });
