import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  coverageBounds,
  coverageInputs,
} from "../../evals/experiments/action-coverage/cases.js";
import { startNative } from "../../evals/experiments/usable/native.js";
import { assertOracle } from "../../evals/experiments/workspace-oracle/node.js";
import type { PortableCommand } from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import {
  checkSafety,
  checkSafetyCoverage,
} from "../../packages/idealis/src/portable.js";
import { compileTlaSource } from "../../packages/idealis/src/tla-source.js";

const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
for (const input of coverageInputs)
  test(`coverage: ${input.name}`, async () => {
    const program = compileTlaSource(core, input);
    const native = startNative();
    const observe = (run: () => unknown) => {
      try {
        return run();
      } catch (error) {
        assert.ok(error instanceof PortableCoreError);
        return { kind: "error", code: error.code, message: error.message };
      }
    };
    try {
      for (const bounds of [
        coverageBounds,
        { ...coverageBounds, maxWork: 1 },
        { ...coverageBounds, maxStates: 1 },
        { ...coverageBounds, maxDepth: 0 },
        { ...coverageBounds, maxTemporalNodes: 1 },
      ]) {
        const command: PortableCommand = {
          version: 1,
          request: { kind: "check-safety-coverage", program, bounds },
        };
        const measured = observe(() => core.request(command));
        assertOracle(await native.request(command), measured);
        const expected = observe(() => checkSafety(core, program, bounds));
        const actual = observe(
          () => checkSafetyCoverage(core, program, bounds).result,
        );
        assertOracle(actual, expected, input.name);
      }
      if (input.expected === "error")
        assert.throws(
          () => checkSafetyCoverage(core, program, coverageBounds),
          PortableCoreError,
        );
      else {
        const report = checkSafetyCoverage(core, program, coverageBounds);
        assert.equal(report.result.status, input.expected);
        assertOracle(
          report.coverage.initial.map((row) => row.name),
          program.initial.map((row) => row.name),
        );
        assertOracle(
          report.coverage.next.map((row) => row.name),
          program.next.map((row) => row.name),
        );
        if (input.name === "counter") {
          assertOracle(
            report.coverage.next.map((row) => ({
              name: row.name,
              attempts: row.attempts,
              enabled: row.enabled,
              generated: row.generated,
              distinct: row.distinct,
            })),
            [
              {
                name: "Advance",
                attempts: 4,
                enabled: 3,
                generated: 3,
                distinct: 3,
              },
              {
                name: "Stay",
                attempts: 4,
                enabled: 4,
                generated: 4,
                distinct: 0,
              },
              {
                name: "Unused",
                attempts: 4,
                enabled: 0,
                generated: 0,
                distinct: 0,
              },
            ],
          );
        }
      }
    } finally {
      await native.close();
    }
  });
