import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { isBuiltin } from "node:module";
import { init, killThreads } from "z3-solver";
import {
  config,
  names,
} from "../../evals/experiments/quint-generated/cases.js";
import { bundleModule } from "../../scripts/bundle-module.js";
import { binaries, withAssets } from "./assets.js";

export const registerWasmHostContract = (
  test: (name: string, run: () => void | Promise<void>) => unknown,
): void => {
  const hostTest = (name: string, run: (origin: string) => Promise<void>) =>
    test(name, () => withAssets(run));

  test("Rust Wasm modules compile with no host imports", async () => {
    for (const [name, path] of Object.entries(binaries)) {
      const module = await WebAssembly.compile(await readFile(path));
      assert.deepEqual(WebAssembly.Module.imports(module), [], name);
      assert.ok(WebAssembly.Module.exports(module).length > 0, name);
    }
  });
  test("portable code and browser workers bundle without Node dependencies", async () => {
    for (const entry of [
      "packages/idealis/src/index.ts",
      "packages/idealis/src/scheduling.ts",
      "packages/idealis/src/portable.ts",
      "packages/idealis/src/tla-source.ts",
      "tests/portable-browser-entry.ts",
      "evals/experiments/parallel-safety/browser-worker.ts",
      "tests/workspace-oracle/browser-entry.ts",
      "tests/testing-core/browser-entry.ts",
      "tests/smt-scripts/browser-entry.ts",
      "tests/alloy-instance/browser-entry.ts",
    ]) {
      const result = await bundleModule(entry);
      assert.ok(result.code.length, entry);
      assert.equal(
        result.inputs.some(
          (name) => isBuiltin(name) || name.includes("__vite-browser-external"),
        ),
        false,
        entry,
      );
    }
  });

  const portable = [
    {
      method: "run",
      expected: {
        source: "invariant-violation",
        backend: "counterexample",
        normal: "matched",
        broken: "mismatch",
        replay: "mismatch",
        steps: 3,
        isolated: false,
        coverageCases: 14,
      },
    },
    {
      method: "runSymmetry",
      expected: {
        status: "complete",
        states: 10,
        generated: 31,
        originalStates: 27,
        records: { status: "complete", states: 56, generated: 337 },
        sets: { status: "complete", states: 20, generated: 121 },
        actions: { status: "complete", states: 10, generated: 19 },
        por: { status: "complete", states: 7, generated: 8 },
        isolated: false,
      },
    },
    {
      method: "runSharedIr",
      expected: {
        version: 2,
        status: "complete",
        states: 1,
        generated: 2,
        bounded: { status: "bounded", reason: "work", work: 256 },
        isolated: false,
      },
    },
    {
      method: "runFrontendCompletion",
      expected: {
        matched: 78,
        rejected: 18,
        evaluation: "evaluation",
        unicode: "complete",
        isolated: false,
      },
    },
    {
      method: "runRecursiveActions",
      expected: { matched: 23, rejected: 1, bounded: 2, isolated: false },
    },
    {
      method: "runCallDepth",
      expected: {
        accepted: { status: "complete", states: 5, generated: 6, work: 862 },
        refusal: {
          code: "protocol",
          message: "Safety evaluation nesting exceeds 256",
        },
        afterRefusal: {
          status: "complete",
          states: 5,
          generated: 6,
          work: 432,
        },
        isolated: false,
      },
    },
    {
      method: "runFrontendSemantics",
      expected: {
        matched: 64,
        rejected: 12,
        modelErrors: 4,
        bounded: 1,
        isolated: false,
      },
    },
  ] as const;
  for (const scenario of portable)
    hostTest(
      `portable Wasm ${scenario.method} retains its verdicts`,
      async (origin) => {
        const entry = await import("../portable-browser-entry.js");
        assert.deepEqual(
          await entry[scenario.method](origin),
          scenario.expected,
        );
      },
    );

  hostTest(
    "Quint generated models retain complete and counterexample outcomes",
    async (origin) => {
      const entry = await import("../browser-entry.js");
      const inputs = (
        await Promise.all(
          names.map(async (name) => {
            const source = await readFile(
              `fixtures/quint-generated/generated/${name}.tla`,
              "utf8",
            );
            return ["safety", "broken"].map((property) => ({
              source,
              config: config(property === "safety" ? "safety" : "broken"),
            }));
          }),
        )
      ).flat();
      const results = await entry.runGenerated(inputs, origin);
      assert.equal(results.length, inputs.length);
      for (const [index, result] of results.entries()) {
        assert.equal(
          result.status,
          index % 2 === 0 ? "complete" : "invariant-violation",
        );
        assert.equal(result.reason, null);
      }
    },
  );
  hostTest(
    "core, shrinking, TLA and real Z3 Wasm run in Node",
    async (origin) => {
      const entry = await import("../browser-entry.js");
      const api = await init();
      try {
        assert.deepEqual(await entry.run(origin, api), {
          model: "bounded",
          replay: "matched",
          property: "failed",
          shrunk: 3,
          tla: "invariant-violation",
          tlaSteps: 2,
          smt: { status: "sat", values: { value: "7" } },
          conformance: "matched",
          isolated: false,
        });
      } finally {
        await killThreads(api.em);
      }
    },
  );
  hostTest(
    "parser Wasm reuses memory after syntax and config errors",
    async (origin) => {
      const entry = await import("../rtla-browser-entry.js");
      assert.deepEqual(await entry.run(origin), {
        name: "Channel",
        unicode: "Forall",
        large: 100_000,
        invalid: "syntax",
        label: "syntax",
        symbols: ["%%", "-.", "^+", "Main"],
        limit: "unsupported",
        config: "config",
        repeat: true,
        isolated: false,
      });
    },
  );
  hostTest(
    "SMT Wasm distinguishes sat, unsat, unknown and errors",
    async (origin) => {
      const entry = await import("../smt-subset/browser-entry.js");
      assert.deepEqual(await entry.run(origin), {
        sat: "sat",
        amount: "7",
        unsat: "unsat",
        unknown: "unknown",
        error: "error",
        repeated: "7",
        isolated: false,
      });
    },
  );
  hostTest(
    "SMT expansion preserves scopes and recovers after an error",
    async (origin) => {
      const entry = await import("../smt-expansion/browser-entry.js");
      assert.deepEqual(await entry.run(origin), {
        checked: 36,
        positive: "3",
        negative: "4",
        error: "error",
        afterError: "3",
        unknown: "unknown",
        restored: "9",
        isolated: false,
      });
    },
  );
  hostTest(
    "SMT scripts preserve checks, budgets and reset behavior",
    async (origin) => {
      const entry = await import("../smt-scripts/browser-entry.js");
      assert.deepEqual(await entry.run(origin), {
        scripts: 7,
        checks: 48,
        bounded: ["sat", "unknown", "unknown"],
        empty: 0,
        error: "error",
        preserved: "9",
        isolated: false,
      });
    },
  );
  hostTest(
    "Alloy Wasm keeps counterexamples, unknown and rejected inputs",
    async (origin) => {
      const entry = await import("../alloy-subset/browser-entry.js");
      assert.deepEqual(await entry.run(origin), {
        status: "counterexample",
        edges: 2,
        unknown: "unknown",
        rejected: "error",
        isolated: false,
      });
    },
  );
  hostTest(
    "Alloy instance replay preserves mismatch and invalid outcomes",
    async (origin) => {
      const entry = await import("../alloy-instance/browser-entry.js");
      assert.deepEqual(await entry.run(origin), {
        status: "counterexample",
        check: "mismatch",
        dag: "matched",
        unknown: "unknown",
        invalid: "invalid",
        isolated: false,
      });
    },
  );
  hostTest(
    "PBT Wasm shrinks and replays an implementation counterexample",
    async (origin) => {
      const entry = await import("../testing-core/browser-entry.js");
      assert.deepEqual(await entry.run(origin), {
        normal: "passed",
        broken: "failed",
        actions: ["add", "crash", "restart"],
        shrink: "minimal",
        replay: "failed",
        fixed: "passed",
        unknown: "inconclusive",
        isolated: false,
      });
    },
  );
  hostTest(
    "controlled actor schedules replay against the native SUT",
    async (origin) => {
      const entry = await import("../controlled-schedules/browser-entry.js");
      const clients =
        await import("../../evals/experiments/controlled-schedules/clients.js");
      const result = await entry.run(origin);
      assert.equal(result.normal.status, "passed");
      assert.equal(result.broken.status, "failed");
      assert.equal(result.broken.shrink?.status, "minimal");
      assert.equal(result.fault.status, "failed");
      assert.equal(result.fixed.status, "passed");
      assert.equal(result.emptyBudget.status, "inconclusive");
      assert.equal(result.called, false);
      assert.deepEqual(result.imports, []);
      assert.ok(result.broken.counterexample);
      const sut = clients.createSut();
      try {
        assert.equal(
          (await sut.evaluate(3, result.broken.counterexample, true)).verdict
            .status,
          "mismatch",
        );
        assert.equal(
          (await sut.evaluate(3, result.broken.counterexample, false)).verdict
            .status,
          "matched",
        );
      } finally {
        await sut.close();
      }
    },
  );
  hostTest(
    "SAT Wasm matches native results and its independent enumerator",
    async (origin) => {
      const entry = await import("../sat-foundation/browser-entry.js");
      const native =
        await import("../../evals/experiments/sat-foundation/native.js");
      const oracle =
        await import("../../evals/experiments/sat-foundation/oracle.js");
      const result = await entry.run(origin);
      assert.equal(result.records.length, 32);
      assert.deepEqual(result.imports, []);
      assert.deepEqual(result.budget, {
        kind: "unsupported",
        reason: "work-budget",
        candidate: "varisat-0.2.2",
      });
      const replies = native.nativeReplies(
        result.records.map((record) => record.input),
      );
      for (const [index, record] of result.records.entries()) {
        assert.deepEqual(record.statuses, record.expected);
        assert.deepEqual(
          oracle.validate(record.input, replies[index]),
          record.expected,
        );
      }
    },
  );
  test("structured fuzz replays actual native request and response records in Wasm", async () => {
    const clients =
      await import("../../evals/experiments/structured-fuzz/clients.js");
    const corpus =
      await import("../../evals/experiments/structured-fuzz/corpus.js");
    const entry = await import("../structured-fuzz/browser-entry.js");
    const audit = await clients.createAudit();
    const records = [];
    try {
      for (const input of corpus.regression)
        records.push(await audit.check(input));
    } finally {
      await audit.close();
    }
    await withAssets(
      async (origin) => {
        const result = await entry.run(origin);
        assert.equal(result.records, corpus.regression.length);
        assert.ok(result.requests > 10);
        assert.deepEqual(result.imports, []);
      },
      { "/cases.json": JSON.stringify(records) },
    );
  });
  hostTest(
    "runtime Wasm matches the TS kernel and sharded results",
    async (origin) => {
      const entry = await import("../runtime-browser-entry.js");
      const result = await entry.run(origin);
      for (const [key, value] of Object.entries({
        matched: true,
        states: 56,
        sharding: "complete",
        workers: 1,
        depth: 8,
      }))
        assert.deepEqual(result[key], value, key);
    },
  );
  hostTest(
    "roracle JSON ABI retains snapshots and signed-zero mismatch",
    async (origin) => {
      const entry = await import("../workspace-oracle/browser-entry.js");
      assert.deepEqual(await entry.run(origin), {
        matched: "matched",
        mismatch: "mismatch",
        isolated: false,
      });
    },
  );
  hostTest(
    "typed oracle Wasm checks replay and host clock against native",
    async (origin) => {
      const native = Number(
        execFileSync(
          "evals/experiments/oracle-comparison/rust/target/release/oracle-comparison",
          ["--probe"],
          { encoding: "utf8" },
        ),
      );
      const wasm = await WebAssembly.instantiateStreaming(
        fetch(`${origin}/typed-oracle.wasm`),
        {},
      );
      const probe = wasm.instance.exports.oracle_probe;
      assert.equal(typeof probe, "function");
      if (typeof probe !== "function") throw new Error("Missing oracle probe");
      assert.equal(probe(), native);
      assert.equal(native, 51);
    },
  );
};
