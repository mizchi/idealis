import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  BackendContractError,
  createRtlcBackend,
  runBackend,
} from "../../packages/idealis/src/backend.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";

const scope = {
  modelId: "ring:v1",
  propertyId: "Safe:v1",
  semantics: "tlc-1.7.4-int32",
  assumptions: [],
};
test("the rtlc adapter executes real Wasm and keeps exhaustion, empty, assumptions and violations distinct", async () => {
  const core = await createPortableWasm(
    await readFile(
      "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
    ),
  );
  const backend = createRtlcBackend(core, { version: "0.1.0-alpha.0" });
  const program = compileTlaSafety({
    source:
      "---- MODULE Ring ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<3 THEN x+1 ELSE 0\nSafe == x<4\n====",
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  });
  const request = {
    format: "modelt.backend-request",
    version: 1,
    scope,
    query: {
      kind: "explicit",
      bounds: { maxDepth: 10, maxStates: 100, maxWork: 1000 },
    },
    input: program,
  } as const;
  assert.equal(
    (await runBackend(backend, request)).outcome.kind,
    "explicit-complete",
  );
  const mismatched: typeof backend = {
    descriptor: backend.descriptor,
    check: async (input) => ({
      ...(await backend.check(input)),
      scope: { ...scope, propertyId: "unrelated" },
    }),
  };
  await assert.rejects(
    () => runBackend(mismatched, request),
    BackendContractError,
  );
  for (const bounds of [
    { ...request.query.bounds, maxDepth: 0 },
    { ...request.query.bounds, maxStates: 1 },
    { ...request.query.bounds, maxWork: 1 },
  ]) {
    const limited = await runBackend(backend, {
      ...request,
      query: { kind: "explicit", bounds },
    });
    assert.equal(limited.outcome.kind, "inconclusive");
  }
  assert.equal(
    (
      await runBackend(backend, {
        ...request,
        input: { ...program, initial: [] },
      })
    ).outcome.kind,
    "empty",
  );
  const fault = compileTlaSafety({
    source:
      "---- MODULE Fault ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x+1\nSafe == x<2\n====",
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  });
  assert.equal(
    (await runBackend(backend, { ...request, input: fault })).outcome.kind,
    "counterexample",
  );
  const assumption = {
    ...program,
    assumptions: [
      {
        name: "Trusted",
        expression: { op: "literal", value: { type: "bool", value: false } },
      },
    ],
  } as const;
  assert.equal(
    (await runBackend(backend, { ...request, input: assumption })).outcome.kind,
    "error",
  );
  assert.equal(
    (
      await runBackend(backend, {
        ...request,
        input: JSON.parse(JSON.stringify({ ...program, version: 2 })),
      })
    ).outcome.kind,
    "error",
  );
  const unsupported = await runBackend(backend, {
    ...request,
    query: { kind: "bounded", maxDepth: 4 },
  });
  assert.ok(
    unsupported.outcome.kind === "inconclusive" &&
      unsupported.outcome.reason === "unsupported",
  );
});
