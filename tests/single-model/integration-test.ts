import assert from "node:assert/strict";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { check } from "tla-ts";
import { cases, fault } from "../../evals/experiments/single-model/cases.js";
import {
  prepare,
  runTool,
  verify,
} from "../../evals/experiments/single-model/runner.js";

const java = process.env.IDEALIS_TLC_JAVA;
const jar = process.env.IDEALIS_TLC_JAR;
assert.ok(java && jar, "Set IDEALIS_TLC_JAVA and IDEALIS_TLC_JAR");
const root = await mkdtemp(join(tmpdir(), "idealis-single-model-"));
await mkdir(".artifacts/single-model-tests", { recursive: true });
for (const input of cases("small"))
  test(`${input.name}: closed graph and delayed invariant faults agree`, async () => {
    for (const [kind, selected] of [
      ["complete", input],
      ["invariant-violation", fault(input)],
    ] as const) {
      for (const route of ["tlc-1", "tlc-8", "native", "wasm"] as const) {
        const directory = join(root, `${input.name}-${kind}-${route}`);
        await prepare(directory, selected);
        const actual = await runTool(route, directory, selected, java, jar);
        await writeFile(
          `.artifacts/single-model-tests/${input.name}-${kind}-${route}.json`,
          JSON.stringify({ input: selected, expected: kind, actual }),
        );
        verify(actual, selected, kind);
      }
      const reference = await check({
        source: selected.source,
        config: selected.config,
        version: "1.7.4",
        workers: 1,
        livenessCheck: "final",
      });
      assert.equal(
        reference.status,
        kind === "complete" ? "success" : "invariant-violation",
      );
      if (kind === "complete") {
        assert.equal(reference.distinctStates, input.states);
        assert.equal(reference.generatedStates, input.generated);
      } else
        assert.ok(reference.violation && reference.violation.trace.length > 1);
    }
  });
