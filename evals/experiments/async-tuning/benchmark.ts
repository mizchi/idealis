import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { checkConformance } from "../../../packages/idealis/src/conformance.js";
import type { PortableResult } from "../../../packages/idealis/src/contracts.js";
import {
  checkFiniteHistory,
  compileFiniteModel,
  createFiniteModel,
} from "../../../packages/idealis/src/finite.js";
import { decodePortableResult } from "../../../packages/idealis/src/portable-wasm.js";
import {
  checkFiniteHistories,
  createPortableWasm,
  loadFiniteModel,
} from "../../../packages/idealis/src/portable.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { bundleModule } from "../../../scripts/bundle-module.js";
import { mutexModel, mutexRegression } from "../usable/mutex.js";
import { digest } from "../usable/provenance.js";
import { createOracle } from "./oracle.js";

const median = (values: readonly number[]) =>
  [...values].sort((left, right) => left - right)[
    Math.floor(values.length / 2)
  ];
export const benchmark = async () => {
  const ring = {
    name: "ring512",
    initial: () => [{ state: 0, observation: 0 }],
    key: String,
    actions: () => [1],
    step: (state: number) => [
      { state: (state + 1) % 512, observation: (state + 1) % 512 },
    ],
  };
  const mutex = compileFiniteModel(mutexModel, {
    maxDepth: 20,
    maxStates: 100,
  });
  const compiledRing = compileFiniteModel(ring, {
    maxDepth: 512,
    maxStates: 512,
  });
  assert.ok(mutex.model && compiledRing.model);
  const fixtures = [
    {
      name: "mutex49",
      model: mutex.model,
      trace: createOracle(mutexModel, mutexRegression),
    },
    {
      name: "ring512",
      model: compiledRing.model,
      trace: createOracle(ring, [1, 1, 1, 1, 1]),
    },
  ];
  const started = performance.now();
  const core = await createPortableWasm(
    await readFile(
      "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
    ),
  );
  const startupMs = performance.now() - started;
  const reports = [];
  for (const fixture of fixtures) {
    const prepared = createFiniteModel(fixture.model);
    const reference = await checkFiniteHistory(prepared, fixture.trace);
    assert.equal(reference.status, "matched");
    const loadSamples = [];
    for (let index = 0; index < 12; index++) {
      const before = performance.now();
      const handle = loadFiniteModel(core, fixture.model);
      const elapsed = performance.now() - before;
      handle.release();
      if (index >= 3) loadSamples.push(elapsed);
    }
    const handle = loadFiniteModel(core, fixture.model);
    const profiles = [];
    for (const batch of [1, 16, 64]) {
      const traces = Array.from({ length: batch }, () => fixture.trace);
      const expected = Array.from({ length: batch }, () => reference);
      const methods: {
        name: string;
        run: () =>
          | Promise<readonly PortableResult[]>
          | readonly PortableResult[];
        samples: number[];
      }[] = [
        {
          name: "ts-prepared",
          run: async () => {
            const results = [];
            for (const trace of traces)
              results.push(await checkFiniteHistory(prepared, trace));
            return results;
          },
          samples: [],
        },
        {
          name: "wasm-wire",
          run: () => checkFiniteHistories(core, fixture.model, traces),
          samples: [],
        },
        {
          name: "wasm-loaded",
          run: () => handle.checkHistories(traces),
          samples: [],
        },
      ];
      for (let sample = -3; sample < 9; sample++)
        for (const method of sample % 2 === 0
          ? methods
          : [...methods].reverse()) {
          const before = performance.now();
          const results = await method.run();
          const elapsed = performance.now() - before;
          assert.deepEqual(results, expected);
          if (sample >= 0) method.samples.push(elapsed);
        }
      for (const method of methods)
        profiles.push({
          mode: method.name,
          batch,
          samples: method.samples,
          medianMs: median(method.samples),
        });
    }
    handle.release();
    const goStarted = performance.now();
    const go = JSON.parse(
      execFileSync(
        ".artifacts/go/finite-bench",
        [
          "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
        ],
        {
          input: JSON.stringify({ model: fixture.model, trace: fixture.trace }),
          maxBuffer: 16 * 1024 * 1024,
        },
      ).toString(),
    );
    const goProcessWallMs = performance.now() - goStarted;
    assert.deepEqual(decodePortableResult(go.reference), reference);
    reports.push({
      name: fixture.name,
      model: fixture.model,
      trace: fixture.trace,
      states: fixture.model.states.length,
      traceSteps: fixture.trace.steps.length,
      modelBytes: Buffer.byteLength(JSON.stringify(fixture.model)),
      loadSamples,
      loadMedianMs: median(loadSamples),
      profiles,
      go: { ...go, goProcessWallMs, reference: undefined },
    });
  }
  const beforeSource = await readFile(
    "fixtures/trace-buffer-before.ts",
    "utf8",
  );
  const baseline = JSON.parse(
    await readFile("fixtures/trace-buffer-before.json", "utf8"),
  );
  assert.equal(digest(beforeSource), baseline.sha256);
  const bundled = await bundleModule(
    "packages/idealis/src/trace-buffer-before.ts",
    { source: beforeSource, platform: "node" },
  );
  const previous: unknown = await import(
    `data:text/javascript;base64,${Buffer.from(bundled.code).toString("base64")}`
  );
  assert.ok(
    isRecord(previous) && typeof previous.checkConformance === "function",
  );
  const oldCheck = previous.checkConformance;
  const traceProfiles = [];
  const model = {
    name: "increment",
    initial: () => [{ state: 0, observation: 0 }],
    key: String,
    actions: () => [1],
    step: (state: number, input: number) => [
      { state: state + input, observation: state + input },
    ],
  };
  const driver = () => {
    let value = 0;
    return {
      reset: () => {
        value = 0;
        return value;
      },
      step: (input: number) => {
        value += input;
        return value;
      },
    };
  };
  for (const length of [64, 4096]) {
    const actions = Array.from({ length }, () => 1);
    const reference = await checkConformance(model, driver(), actions);
    const methods = [
      {
        name: "copy-prefix",
        run: () => oldCheck(model, driver(), actions),
        samples: [] as number[],
      },
      {
        name: "buffer",
        run: () => checkConformance(model, driver(), actions),
        samples: [] as number[],
      },
    ];
    for (let sample = -3; sample < 9; sample++)
      for (const method of sample % 2 === 0
        ? methods
        : [...methods].reverse()) {
        const before = performance.now();
        const result = await method.run();
        const elapsed = performance.now() - before;
        assert.deepEqual(result, reference);
        if (sample >= 0) method.samples.push(elapsed);
      }
    traceProfiles.push({
      length,
      profiles: methods.map((method) => ({
        mode: method.name,
        samples: method.samples,
        medianMs: median(method.samples),
      })),
    });
  }
  return { startupMs, reports, traceProfiles, baseline };
};
