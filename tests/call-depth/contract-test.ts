import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { after, test } from "node:test";
import {
  type DepthCase,
  depthCases,
  type Records0119,
} from "../../evals/experiments/call-depth/expectations-0119.js";
import {
  depthLimit,
  hopLevels,
} from "../../evals/experiments/call-depth/sources.js";
import {
  type Check,
  type Compile,
  nativeFrontend,
} from "../../evals/experiments/nesting-chains/frontends.js";
import {
  bounds,
  type Frontend,
  type Input,
  type Outcome,
  observe,
} from "../../evals/experiments/nesting-chains/outcome.js";
import { startNative } from "../../evals/experiments/usable/native.js";
import type { SafetyProgram } from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import { nativePath, wasmPath } from "../ci/tlc-build.js";
import { normalize } from "../tlc-differential/operator-sharing-support.js";

const directory = "fixtures/call-depth";
const readJson = async (path: string): Promise<unknown> =>
  JSON.parse(await readFile(path, "utf8"));
const records = (await readJson(
  `${directory}/records-0119.json`,
)) as Records0119;
const cases = (await readJson(
  `${directory}/cases-0119.json`,
)) as readonly DepthCase[];

const core = await createPortableWasm(await readFile(wasmPath));
const native = startNative(nativePath);
after(() => native.close());

const guarded = <Value>(
  run: () => Value,
): Value | { kind: "error"; message: string } => {
  try {
    return run();
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error", message: error.message };
  }
};

const wasm = {
  compile: async (input: Input): Promise<SafetyProgram> => {
    const reply = core.request({
      version: 1,
      request: { kind: "compile-tla", ...input },
    });
    assert.equal(reply.kind, "compiled");
    return reply.program;
  },
  check: async (program: SafetyProgram): Promise<unknown> =>
    guarded(() =>
      core.request({
        version: 1,
        request: { kind: "check-safety", program, bounds },
      }),
    ),
};
const typescript: Compile = async (input) => compileTlaSafety(input);
const nativeRust = nativeFrontend((command) => native.request(command));

interface Path {
  readonly frontend: Frontend;
  readonly label: string;
  readonly compile: Compile;
  readonly check: Check;
}
const paths: readonly Path[] = [
  {
    frontend: "ts",
    label: "TS frontend to Wasm",
    compile: typescript,
    check: wasm.check,
  },
  {
    frontend: "ts",
    label: "TS frontend to native",
    compile: typescript,
    check: nativeRust.check,
  },
  {
    frontend: "rust",
    label: "Rust frontend Wasm",
    compile: wasm.compile,
    check: wasm.check,
  },
  {
    frontend: "rust",
    label: "Rust frontend native",
    compile: nativeRust.compile,
    check: nativeRust.check,
  },
];

test("call depth fixtures follow from the recorded baselines and the plan", () => {
  assert.deepEqual(cases, depthCases(records));
  assert.equal(cases.length, 31);
});

for (const entry of cases)
  test(`call depth ${entry.group}: ${entry.name}`, async () => {
    for (const irVersion of [1, 2] as const) {
      const input: Input = {
        source: entry.source,
        config: entry.config,
        irVersion,
      };
      const programs: Partial<Record<string, SafetyProgram>> = {};
      for (const path of paths) {
        const expected = entry.expect[path.frontend][`v${irVersion}`];
        const observation = await observe(path.compile, path.check, input);
        assert.deepEqual(
          observation.outcome,
          expected,
          `${path.label} v${irVersion}`,
        );
        if (observation.program) programs[path.label] = observation.program;
      }
      const wasmIr = programs["Rust frontend Wasm"];
      const nativeIr = programs["Rust frontend native"];
      if (wasmIr && nativeIr)
        assert.deepEqual(wasmIr, nativeIr, `Wasm and native IR v${irVersion}`);
      const tsIr = programs["TS frontend to Wasm"];
      if (tsIr && wasmIr)
        assert.deepEqual(
          normalize(tsIr),
          normalize(wasmIr),
          `TS and Rust frontend IR v${irVersion}`,
        );
    }
  });

const find = (name: string): DepthCase => {
  const found = cases.find((entry) => entry.name === name);
  assert.ok(found, name);
  return found;
};
const completes = (outcome: Outcome): boolean =>
  outcome.stage === "result" && outcome.status === "complete";
const refusedAtTheLimit = (outcome: Outcome): boolean =>
  outcome.stage === "check" &&
  outcome.message === `Safety evaluation nesting exceeds ${depthLimit}`;

test("call depth layered calls complete up to depth 255 and refuse depth 256 on every path", () => {
  for (const frontend of ["ts", "rust"] as const)
    for (const prefix of ["hop", "hop-invariant"]) {
      if (prefix === "hop" && frontend === "ts") continue;
      for (const levels of hopLevels.filter((count) => count < 84))
        assert.ok(
          completes(find(`${prefix}-${levels}`).expect[frontend].v2),
          `${frontend} ${prefix}-${levels}`,
        );
      for (const levels of hopLevels.filter((count) => count >= 84))
        assert.ok(
          refusedAtTheLimit(find(`${prefix}-${levels}`).expect[frontend].v2),
          `${frontend} ${prefix}-${levels}`,
        );
    }
});

test("call depth keeps the recursive limit and the frontend limits", () => {
  for (const name of [
    "recursion-loop",
    "recursion-sum-200",
    "recursion-from-hop-50",
  ]) {
    const expected = find(name).expect.rust.v2;
    assert.ok(
      expected.stage === "result" &&
        expected.status === "bounded" &&
        expected.reason === "evaluation",
      name,
    );
  }
  for (const levels of hopLevels)
    assert.deepEqual(find(`hop-${levels}`).expect.rust.v1, {
      stage: "compile",
      message: "expression or operator nesting exceeds 40",
    });
  assert.deepEqual(find("unchanged-9").expect.rust.v2, {
    stage: "compile",
    message: "Safety branch exceeds 256 steps",
  });
});

test("a typed refusal leaves the same Wasm instance able to check the next request", async () => {
  const refused = find("hop-invariant-84");
  const input: Input = {
    source: refused.source,
    config: refused.config,
    irVersion: 2,
  };
  const first = await observe(wasm.compile, wasm.check, input);
  assert.ok(refusedAtTheLimit(first.outcome));
  const small = find("hop-invariant-40");
  const next = await observe(wasm.compile, wasm.check, {
    source: small.source,
    config: small.config,
    irVersion: 2,
  });
  assert.deepEqual(next.outcome, small.expect.rust.v2);
});
