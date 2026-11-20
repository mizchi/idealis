import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import type {
  PortableCommand,
  TlaSourceInput,
} from "../../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { checkSafety } from "../../../packages/idealis/src/safety.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { bounds } from "../collections-temporal/cases.js";
import { runTlc } from "../collections-temporal/tlc.js";
import { classifyTlc } from "../tlc-differential/compare.js";
import { differential } from "../tlc-differential/differential.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";
import { type ActionCase, generatedCases } from "./generated.js";

interface PropertyPlan {
  readonly developmentSeed: number;
  readonly confirmationSeed: number;
  readonly casesPerSeed: number;
  readonly maxDepth: number;
  readonly maxStates: number;
  readonly maxWork: number;
}

const [directory, java, jar] = process.argv.slice(2);
assert.ok(directory && java && jar);
await mkdir(directory);
const paths = [
  "crates/rtla-compiler/tests/recursive-actions.json",
  "evals/plans/0071-recursive-action-properties.json",
  "evals/experiments/recursive-actions/execute.ts",
  "evals/experiments/recursive-actions/generated.ts",
  "evals/experiments/tlc-differential/differential.ts",
  "evals/experiments/collections-temporal/tlc.ts",
  "crates/idealis-core/target/release/idealis-core",
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  jar,
  "evals/experiments/collections-temporal/cases.ts",
  "packages/idealis/src/safety-program.ts",
  "packages/idealis/src/contracts.ts",
  "packages/idealis/src/tla-source.ts",
  "dist/evals/experiments/recursive-actions/execute.js",
  "dist/evals/experiments/recursive-actions/generated.js",
];
const sources = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const plan: PropertyPlan = JSON.parse(
  await readFile("evals/plans/0071-recursive-action-properties.json", "utf8"),
);
const cases: readonly ActionCase[] = JSON.parse(
  await readFile("crates/rtla-compiler/tests/recursive-actions.json", "utf8"),
);
const inputs = [
  ...cases,
  ...generatedCases(plan.developmentSeed, plan.casesPerSeed),
  ...generatedCases(plan.confirmationSeed, plan.casesPerSeed),
];
const save = (name: string, value: unknown) =>
  writeFile(`${directory}/${name}`, `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  plan: "0071-recursive-actions",
  sources,
  inputs,
  bounds: {
    direct: {
      maxDepth: plan.maxDepth,
      maxStates: plan.maxStates,
      maxWork: plan.maxWork,
      maxTemporalNodes: 10000,
    },
    differential: bounds,
  },
  scope:
    "Exact native/Wasm source/IR responses, independent TLC verdict/counts and forced witnesses. Rejected and bounded cases are separate; generated faults are deliberate.",
});
const wasm = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const native = startNative("crates/idealis-core/target/release/idealis-core");
const runtimeBounds = {
  maxDepth: plan.maxDepth,
  maxStates: plan.maxStates,
  maxWork: plan.maxWork,
  maxTemporalNodes: 10000,
};
const request = (command: PortableCommand) => {
  try {
    return wasm.request(command);
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error", code: error.code, message: error.message };
  }
};
const results = [];
try {
  for (const [index, input] of inputs.entries()) {
    const source: TlaSourceInput = {
      source: input.source,
      config: input.config,
      irVersion: 2,
      ...(input.modules ? { modules: input.modules } : {}),
    };
    const command: PortableCommand = {
      version: 1,
      request: { kind: "compile-tla", ...source },
    };
    const compiled = request(command);
    assert.deepEqual(await native.request(command), compiled, input.name);
    await mkdir(`${directory}/${index}`);
    if (input.status === "error") {
      assert.ok(isRecord(compiled) && compiled.kind === "error");
      const reference = await runTlc(
        java,
        jar,
        `${directory}/${index}/reference`,
        input,
        "Differential",
      );
      assert.equal(classifyTlc(reference), "input-error", input.name);
      const result = {
        name: input.name,
        outcome: "rejected",
        compiled,
        reference,
      };
      results.push(result);
      await save(`${index}/result.json`, result);
      continue;
    }
    const program = compileTlaSource(wasm, input);
    const checked = checkSafety(wasm, program, runtimeBounds);
    assert.deepEqual(
      await native.request({
        version: 1,
        request: { kind: "check-safety", program, bounds: runtimeBounds },
      }),
      { kind: "safety", result: checked },
    );
    assert.equal(checked.status, input.status, input.name);
    const partial = checkSafety(wasm, program, {
      ...runtimeBounds,
      maxWork: 1,
    });
    assert.equal(partial.status, "bounded");
    assert.equal(partial.reason, "work");
    assert.equal(partial.work, 1);
    if (input.status === "bounded") {
      assert.equal(checked.reason, "evaluation");
      assert.equal(checked.violation, null);
      const result = { name: input.name, outcome: "bounded", checked, partial };
      results.push(result);
      await save(`${index}/result.json`, result);
      continue;
    }
    assert.equal(checked.states, input.states, input.name);
    assert.equal(checked.generated, input.generated, input.name);
    const report = await differential(
      { ...input, family: "control" },
      wasm,
      native,
      java,
      jar,
      `${directory}/${index}`,
      (entry) => compileTlaSource(wasm, { ...input, ...entry }),
    );
    const result = {
      ...report,
      name: input.name,
      outcome: report.differences.length ? "mismatch" : "matched",
      partial,
    };
    results.push(result);
    await save(`${index}/result.json`, result);
    console.log(input.name, result.outcome);
  }
} finally {
  await native.close();
}
await save("results.json", results);
for (const source of sources)
  assert.equal(digest(await readFile(source.path)), source.sha256, source.path);
const differences = results.filter((result) => result.outcome === "mismatch");
await save("summary.json", {
  cases: results.length,
  matched: results.filter((result) => result.outcome === "matched").length,
  rejected: results.filter((result) => result.outcome === "rejected").length,
  bounded: results.filter((result) => result.outcome === "bounded").length,
  differences,
});
assert.equal(differences.length, 0);
