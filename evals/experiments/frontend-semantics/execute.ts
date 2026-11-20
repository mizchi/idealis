import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import type { PortableCommand } from "../../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { runTlc } from "../collections-temporal/tlc.js";
import { classifyTlc } from "../tlc-differential/compare.js";
import { differential } from "../tlc-differential/differential.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";
import type { FrontendCase } from "./cases.js";
import { readSnapshotSource } from "./snapshot-source.js";

const [directory, java, jar] = process.argv.slice(2);
assert.ok(directory && java && jar);
await mkdir(directory);
const paths = [
  "crates/rtla-compiler/tests/frontend-semantics.json",
  "evals/plans/0072-frontend-properties-2.json",
  "evals/experiments/frontend-semantics/cases.ts",
  "evals/experiments/frontend-semantics/lazy-cases.ts",
  "evals/experiments/frontend-semantics/value-cases.ts",
  "evals/experiments/frontend-semantics/generated.ts",
  "evals/experiments/frontend-semantics/execute.ts",
  "evals/experiments/frontend-semantics/snapshot-source.ts",
  "evals/experiments/tlc-differential/differential.ts",
  "evals/experiments/collections-temporal/tlc.ts",
  "packages/idealis/src/safety-program.ts",
  "packages/idealis/src/contracts.ts",
  "packages/idealis/src/tla-source.ts",
  "crates/idealis-core/target/release/idealis-core",
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  jar,
];
const snapshot = process.env.IDEALIS_FRONTEND_SNAPSHOT;
assert.ok(snapshot, "Freeze native/Wasm sources with runtime.ts first");
const provenance: {
  readonly sources: readonly {
    readonly path: string;
    readonly sha256: string;
  }[];
} = JSON.parse(await readFile(`${snapshot}/manifest.json`, "utf8"));
const build: {
  readonly status: string;
  readonly binaries: readonly {
    readonly path: string;
    readonly sha256: string;
  }[];
} = JSON.parse(await readFile(`${snapshot}/build.json`, "utf8"));
assert.equal(build.status, "complete");
for (const binary of build.binaries)
  assert.equal(digest(await readFile(binary.path)), binary.sha256, binary.path);
paths.push(`${snapshot}/manifest.json`, `${snapshot}/build.json`);
const sources = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const inputs: readonly FrontendCase[] = JSON.parse(
  await readFile(paths[0] ?? "", "utf8"),
);
const save = (name: string, value: unknown) =>
  writeFile(`${directory}/${name}`, `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const bounds = {
  maxDepth: 1000,
  maxStates: 2000,
  maxWork: 1_000_000,
  maxTemporalNodes: 10000,
};
await save("manifest.json", {
  plan: "0072-frontend-semantics",
  sources,
  inputs,
  bounds,
  scope:
    "Native/Wasm compile and check envelopes. Independent TLC verdict/counts/forced witnesses via roracle. Input errors, model errors and bounded recursion are separate. Generated faults are deliberate.",
});
const wasm = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const native = startNative("crates/idealis-core/target/release/idealis-core");
const request = (command: PortableCommand): unknown => {
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
    const path = `${directory}/${index}`;
    await mkdir(path);
    try {
      const source = {
        source: input.source,
        config: input.config,
        irVersion: input.irVersion,
      };
      const compiled = request({
        version: 1,
        request: { kind: "compile-tla", ...source },
      });
      assert.deepEqual(
        await native.request({
          version: 1,
          request: { kind: "compile-tla", ...source },
        }),
        compiled,
        input.name,
      );
      if (input.expected === "input-error") {
        assert.ok(isRecord(compiled) && compiled.kind === "error");
        const reference = await runTlc(
          java,
          jar,
          `${path}/reference`,
          source,
          "Differential",
        );
        assert.equal(classifyTlc(reference), "input-error");
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
      const program = compileTlaSource(wasm, source);
      const checked = request({
        version: 1,
        request: { kind: "check-safety", program, bounds },
      });
      assert.deepEqual(
        await native.request({
          version: 1,
          request: { kind: "check-safety", program, bounds },
        }),
        checked,
        input.name,
      );
      if (input.expected === "evaluation-error") {
        assert.ok(isRecord(checked) && checked.kind === "error");
        const reference = await runTlc(
          java,
          jar,
          `${path}/reference`,
          source,
          "Differential",
        );
        assert.equal(classifyTlc(reference), "model-error");
        const result = {
          name: input.name,
          outcome: "model-error",
          checked,
          reference,
        };
        results.push(result);
        await save(`${index}/result.json`, result);
        continue;
      }
      assert.ok(
        isRecord(checked) &&
          checked.kind === "safety" &&
          isRecord(checked.result),
      );
      assert.equal(
        checked.result.status,
        input.expected === "temporal-violation"
          ? "liveness-violation"
          : input.expected,
      );
      const partial = request({
        version: 1,
        request: {
          kind: "check-safety",
          program,
          bounds: { ...bounds, maxWork: 1 },
        },
      });
      assert.ok(
        isRecord(partial) &&
          partial.kind === "safety" &&
          isRecord(partial.result),
      );
      assert.equal(partial.result.status, "bounded");
      assert.equal(partial.result.reason, "work");
      assert.equal(partial.result.work, 1);
      if (input.expected === "bounded") {
        assert.equal(checked.result.reason, "evaluation");
        assert.equal(checked.result.violation, null);
        const result = {
          name: input.name,
          outcome: "bounded",
          checked,
          partial,
        };
        results.push(result);
        await save(`${index}/result.json`, result);
        continue;
      }
      const report = await differential(
        { ...input, family: "control" },
        wasm,
        native,
        java,
        jar,
        path,
        (entry) => compileTlaSource(wasm, { ...source, ...entry }),
      );
      assert.deepEqual(report.differences, []);
      const result = { name: input.name, outcome: "matched", report, partial };
      results.push(result);
      await save(`${index}/result.json`, result);
      console.log(input.name, "matched");
    } catch (error) {
      const result = {
        name: input.name,
        outcome: "failed",
        assertion: error instanceof Error ? error.stack : String(error),
      };
      results.push(result);
      await save(`${index}/failure.json`, result);
      console.log(input.name, "failed", result.assertion);
    }
  }
} finally {
  await native.close();
}
for (const source of provenance.sources)
  assert.equal(
    digest(await readSnapshotSource(source.path)),
    source.sha256,
    source.path,
  );
for (const source of sources)
  assert.equal(digest(await readFile(source.path)), source.sha256, source.path);
await save("results.json", results);
const summary = {
  cases: results.length,
  matched: results.filter((result) => result.outcome === "matched").length,
  rejected: results.filter((result) => result.outcome === "rejected").length,
  modelErrors: results.filter((result) => result.outcome === "model-error")
    .length,
  bounded: results.filter((result) => result.outcome === "bounded").length,
  failures: results.filter((result) => result.outcome === "failed"),
};
await save("summary.json", summary);
console.log(summary);
assert.equal(summary.failures.length, 0);
