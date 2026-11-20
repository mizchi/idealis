import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { test } from "node:test";
import {
  counterFaults,
  createFaultyCounter,
} from "../../evals/experiments/harness/counter-faults.js";
import { runNative } from "../../evals/experiments/runtime/native.js";
import {
  compileCounterTable,
  decodeCases,
  runTable,
} from "../../evals/experiments/runtime/table.js";
import {
  createWasmKernel,
  encodeTable,
} from "../../evals/experiments/runtime/wasm.js";
import { counterModel } from "../../examples/counter-model.js";
import { checkConformance } from "../../packages/idealis/src/index.js";

test("Rust native and Wasm match the TS packed kernel and replay against independent models", async () => {
  const table = compileCounterTable();
  await mkdir(".artifacts/runtime", { recursive: true });
  await writeFile(".artifacts/runtime/table.bin", encodeTable(table));
  const kernel = await createWasmKernel(
    await readFile(
      "evals/experiments/rust-probe/target/wasm32-unknown-unknown/release/idealis_runtime_probe.wasm",
    ),
    table,
  );
  const detected = new Set<string>();
  for (let seed = 61; seed <= 100; seed++) {
    const options = {
      seed,
      operations: 512,
      depth: 16,
      strategy: seed % 2,
      repeats: 1,
      trace: true,
    };
    const reference = runTable(table, options);
    const actual = kernel.run(options);
    assert.deepEqual(actual, reference);
    const native = await runNative(
      resolve(
        "evals/experiments/rust-probe/target/release/idealis-runtime-probe",
      ),
      ".artifacts/runtime/table.bin",
      { ...options, trace: false },
    );
    assert.deepEqual(native.result, { ...reference, trace: [] });
    for (const fault of ["none", ...counterFaults] as const) {
      for (const actions of decodeCases(table, actual.trace)) {
        const outcome = await checkConformance(
          counterModel,
          createFaultyCounter(fault),
          actions,
        );
        assert.notEqual(outcome.status, "invalid");
        if (fault === "none") assert.equal(outcome.status, "matched");
        if (outcome.status === "mismatch") detected.add(fault);
      }
    }
  }
  assert.deepEqual([...detected].sort(), [...counterFaults].sort());
});
