import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import {
  completeInduction,
  createRtlcBackend,
  parseBackendResult,
  runBackend,
} from "../../../packages/idealis/src/backend.js";
import type {
  BackendResult,
  BackendScope,
  PortableCore,
} from "../../../packages/idealis/src/contracts.js";
import {
  PortableCoreError,
  toPortableValue,
} from "../../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyResult } from "../../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { cases } from "../apalache-comparison/cases.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { digest } from "../usable/provenance.js";

const directory = process.argv[2];
assert.ok(directory);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const base = await fingerprint();
const additional = [
  "evals/plans/0028-backend-contract.md",
  "evals/experiments/backend-contract/evaluate.ts",
  "tests/backend-contract/integration-test.ts",
  "evals/experiments/apalache-comparison/cases.ts",
];
const sources = [
  ...base.sources,
  ...(await Promise.all(
    additional.map(async (path) => ({
      path,
      sha256: digest(await readFile(path)),
    })),
  )),
].sort((left, right) => left.path.localeCompare(right.path));
const sourceVersion = digest(JSON.stringify(sources));
const saved = "evals/runs/0027-apalache-pilot";
const pilot: unknown = JSON.parse(
  await readFile(join(saved, "result.json"), "utf8"),
);
assert.ok(
  isRecord(pilot) && pilot.status === "complete" && Array.isArray(pilot.files),
);
for (const entry of pilot.files) {
  assert.ok(
    isRecord(entry) &&
      typeof entry.path === "string" &&
      typeof entry.sha256 === "string",
  );
  assert.equal(digest(await readFile(join(saved, entry.path))), entry.sha256);
}
const samples: unknown = JSON.parse(
  await readFile(join(saved, "samples.json"), "utf8"),
);
assert.ok(Array.isArray(samples) && samples.length === 72);
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "0028-backend-contract",
      sourceVersion,
      sources,
      parent: saved,
      parentResultSha256: digest(await readFile(join(saved, "result.json"))),
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const native: PortableCore = {
  request: (command) => {
    const execution = spawnSync(
      "crates/idealis-core/target/release/idealis-core",
      [],
      { input: `${JSON.stringify(command)}\n`, encoding: "utf8" },
    );
    if (execution.error) throw execution.error;
    assert.equal(execution.status, 0, execution.stderr);
    const message: unknown = JSON.parse(execution.stdout);
    assert.ok(isRecord(message));
    if (message.kind === "error") {
      assert.ok(
        (message.code === "protocol" || message.code === "value") &&
          typeof message.message === "string",
      );
      throw new PortableCoreError(message.code, message.message);
    }
    assert.equal(message.kind, "safety");
    return { kind: "safety", result: decodeSafetyResult(message.result) };
  },
};
const nativeBackend = createRtlcBackend(native, { version: "0.1.0-alpha.0" });
const wasmBackend = createRtlcBackend(
  await createPortableWasm(
    await readFile(
      "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
    ),
  ),
  { version: "0.1.0-alpha.0" },
);
const records: BackendResult[] = [];
try {
  for (const input of cases) {
    const program = compileTlaSafety(input);
    const scope: BackendScope = {
      modelId: digest(JSON.stringify([input.source, input.config])),
      propertyId: digest(JSON.stringify(program.invariants)),
      semantics: "tlc-1.7.4-int32",
      assumptions: [],
    };
    const request = {
      format: "modelt.backend-request",
      version: 1,
      scope,
      query: {
        kind: "explicit",
        bounds: { maxDepth: input.depth, maxStates: 65536, maxWork: 2000000 },
      },
      input: program,
    } as const;
    const nativeResult = await runBackend(nativeBackend, request);
    const wasmResult = await runBackend(wasmBackend, request);
    assert.deepEqual(wasmResult, nativeResult);
    const expected =
      input.expected === "complete"
        ? "explicit-complete"
        : input.expected === "bounded"
          ? "inconclusive"
          : "counterexample";
    assert.equal(nativeResult.outcome.kind, expected, input.name);
    records.push(nativeResult, wasmResult);
    for (const sample of samples) {
      assert.ok(isRecord(sample));
      if (sample.name !== input.name || sample.backend !== "apalache") continue;
      assert.ok(
        sample.outcome === "counterexample" ||
          sample.outcome === "bounded-no-counterexample",
      );
      const outcome =
        sample.outcome === "bounded-no-counterexample"
          ? { kind: "bounded-no-counterexample", maxDepth: input.depth }
          : (() => {
              assert.ok(Array.isArray(sample.witness));
              return {
                kind: "counterexample",
                witness: {
                  kind: "behavior",
                  violation: "invariant-violation",
                  name: "Safe",
                  variables: program.variables,
                  trace: sample.witness.map(
                    (state: unknown, index: number) => ({
                      action: index === 0 ? null : "Next",
                      values: [toPortableValue(state)],
                    }),
                  ),
                },
              };
            })();
      records.push(
        parseBackendResult({
          format: "modelt.backend-result",
          version: 1,
          backend: { id: "apalache", version: "0.63.0" },
          scope: { ...scope, semantics: "apalache-0.63.0-smt-int" },
          query: { kind: "bounded", maxDepth: input.depth },
          initialization: "nonempty",
          outcome,
        }),
      );
    }
  }
  assert.equal(records.length, 36);
  const ring = records.find(
    (entry) =>
      entry.query.kind === "explicit" &&
      entry.outcome.kind === "explicit-complete" &&
      entry.outcome.states === 16,
  );
  assert.ok(
    ring && Array.isArray(pilot.induction) && pilot.induction.length === 2,
  );
  const obligations = pilot.induction.map((entry: unknown, index: number) => {
    assert.ok(isRecord(entry) && entry.outcome === "bounded-no-counterexample");
    return parseBackendResult({
      format: "modelt.backend-result",
      version: 1,
      backend: { id: "apalache", version: "0.63.0" },
      scope: { ...ring.scope, semantics: "apalache-0.63.0-smt-int" },
      query: {
        kind: "induction",
        invariantId: ring.scope.propertyId,
        obligation: index === 0 ? "init" : "step",
      },
      initialization: index === 0 ? "nonempty" : "unknown",
      outcome: { kind: "obligation-holds" },
    });
  });
  const induction = completeInduction(obligations);
  assert.equal(induction.outcome.kind, "inductive");
  const partial = completeInduction(obligations.slice(0, 1));
  assert.equal(partial.outcome.kind, "inconclusive");
  for (const entry of [...records, induction, partial])
    assert.deepEqual(
      parseBackendResult(JSON.parse(JSON.stringify(entry))),
      entry,
    );
  await writeFile(
    join(directory, "records.json"),
    JSON.stringify({ records, induction, partial }, null, 2),
    { flag: "wx" },
  );
  for (const entry of sources)
    assert.equal(digest(await readFile(entry.path)), entry.sha256);
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      {
        status: "complete",
        sourceVersion,
        nativeWasmPairs: 6,
        apalacheBoundedResults: 24,
        induction: "inductive",
        partial: "inconclusive",
        notes:
          "Initial nonemptiness of these fixed pilot models was confirmed by concrete enumeration. Saved Apalache replies are adapted, not a generic live Apalache implementation.",
        files: await Promise.all(
          ["manifest.json", "records.json"].map(async (path) => ({
            path,
            sha256: digest(await readFile(join(directory, path))),
          })),
        ),
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "backend-contract", status: "complete", sourceVersion })}\n`,
  );
  console.log(
    "6 native/Wasm pairs, 24 saved bounded results and induction evidence validated",
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", message: String(error), records },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "backend-contract", status: "error", sourceVersion })}\n`,
  );
  throw error;
}
