import { readFile } from "node:fs/promises";
import { createServer, type Server } from "node:http";
import { expect, test } from "@playwright/test";
import { cases } from "../evals/experiments/collections-temporal/cases.js";
import { compileTlaSafety } from "../packages/idealis/src/tla-safety.js";
import { bundleModule } from "../scripts/bundle-module.js";

let server: Server | undefined;
let origin: string;
test.beforeAll(async () => {
  const bundle = await bundleModule("tests/portable-browser-entry.ts", {
    globalName: "portableBrowser",
  });
  const wasm = await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  );
  const safety = JSON.stringify(
    compileTlaSafety({
      source:
        "---- MODULE BrowserSafety ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nSafe == x < 3\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
    }),
  );
  const complex = JSON.stringify(
    cases
      .filter((input) =>
        [
          "set-fault",
          "function-updates",
          "sequence-queue",
          "multiple-eventualities",
          "SF-intermittent",
          "temporal-node-cut",
        ].includes(input.name),
      )
      .map((input) => ({
        program: compileTlaSafety(input),
        bounds: input.bounds,
        expected: input.expected,
      })),
  );
  const finite = await readFile(
    "crates/rtla-compiler/tests/finite-support.json",
  );
  const modules = await readFile(
    "crates/rtla-compiler/tests/modules-support.json",
  );
  const completion = await readFile(
    "crates/rtla-compiler/tests/frontend-completion.json",
  );
  const recursive = await readFile(
    "crates/rtla-compiler/tests/recursive-actions.json",
  );
  const frontend = await readFile(
    "crates/rtla-compiler/tests/frontend-semantics.json",
  );
  const operators = await readFile(
    "crates/rtla-compiler/tests/operators-support.json",
  );
  server = createServer((request, response) => {
    response.setHeader(
      "Content-Type",
      request.url === "/core.wasm"
        ? "application/wasm"
        : request.url === "/bundle.js"
          ? "text/javascript; charset=utf-8"
          : request.url === "/safety.json" || request.url === "/complex.json"
            ? "application/json"
            : "text/html",
    );
    response.end(
      request.url === "/core.wasm"
        ? wasm
        : request.url === "/bundle.js"
          ? Buffer.from(bundle.code)
          : request.url === "/safety.json"
            ? safety
            : request.url === "/complex.json"
              ? complex
              : request.url === "/finite.json"
                ? finite
                : request.url === "/modules.json"
                  ? modules
                  : request.url === "/operators.json"
                    ? operators
                    : request.url === "/completion.json"
                      ? completion
                      : request.url === "/recursive-actions.json"
                        ? recursive
                        : request.url === "/frontend-semantics.json"
                          ? frontend
                          : '<!doctype html><title>idealis portable core</title><script src="/bundle.js"></script>',
    );
  });
  await new Promise<void>((resolve) => server?.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string")
    throw new Error("Missing server address");
  origin = `http://127.0.0.1:${address.port}`;
});
test.afterAll(async () => {
  if (server)
    await new Promise<void>((resolve, reject) =>
      server?.close((error) => (error ? reject(error) : resolve())),
    );
});
test("runs the shared Rust core in Chromium without host imports or cross-origin isolation", async ({
  page,
}) => {
  await page.goto(origin);
  const result = await page.evaluate(async () => {
    const api: unknown = Reflect.get(globalThis, "portableBrowser");
    if (
      typeof api !== "object" ||
      api === null ||
      !("run" in api) ||
      typeof api.run !== "function"
    )
      throw new Error("Missing portable browser API");
    return api.run();
  });
  expect(result).toEqual({
    source: "invariant-violation",
    backend: "counterexample",
    normal: "matched",
    broken: "mismatch",
    replay: "mismatch",
    steps: 3,
    isolated: false,
    coverageCases: 14,
  });
});

test("reduces a model-value symmetry group in Chromium without host imports", async ({
  page,
}) => {
  await page.goto(origin);
  const result = await page.evaluate(async () => {
    const api: unknown = Reflect.get(globalThis, "portableBrowser");
    if (
      typeof api !== "object" ||
      api === null ||
      !("runSymmetry" in api) ||
      typeof api.runSymmetry !== "function"
    )
      throw new Error("Missing browser symmetry API");
    return api.runSymmetry();
  });
  expect(result).toEqual({
    status: "complete",
    states: 10,
    generated: 31,
    originalStates: 27,
    records: { status: "complete", states: 56, generated: 337 },
    sets: { status: "complete", states: 20, generated: 121 },
    actions: { status: "complete", states: 10, generated: 19 },
    por: { status: "complete", states: 7, generated: 8 },
    isolated: false,
  });
});

test("executes shared operator IR and bounds exponential work in Chromium", async ({
  page,
}) => {
  await page.goto(origin);
  const result = await page.evaluate(async () => {
    const api: unknown = Reflect.get(globalThis, "portableBrowser");
    if (
      typeof api !== "object" ||
      api === null ||
      !("runSharedIr" in api) ||
      typeof api.runSharedIr !== "function"
    )
      throw new Error("Missing shared IR browser API");
    return api.runSharedIr();
  });
  expect(result).toEqual({
    version: 2,
    status: "complete",
    states: 1,
    generated: 2,
    bounded: { status: "bounded", reason: "work", work: 256 },
    isolated: false,
  });
});

test("executes the completed Rust frontend and recursion budget in Chromium", async ({
  page,
}) => {
  await page.goto(origin);
  const result = await page.evaluate(async () => {
    const api: unknown = Reflect.get(globalThis, "portableBrowser");
    if (
      typeof api !== "object" ||
      api === null ||
      !("runFrontendCompletion" in api) ||
      typeof api.runFrontendCompletion !== "function"
    )
      throw new Error("Missing frontend browser API");
    return api.runFrontendCompletion();
  });
  expect(result).toEqual({
    matched: 78,
    rejected: 18,
    evaluation: "evaluation",
    unicode: "complete",
    isolated: false,
  });
});

test("generates recursive assignment actions and keeps budget exhaustion inconclusive in Chromium", async ({
  page,
}) => {
  await page.goto(origin);
  const result = await page.evaluate(async () => {
    const api: unknown = Reflect.get(globalThis, "portableBrowser");
    if (
      typeof api !== "object" ||
      api === null ||
      !("runRecursiveActions" in api) ||
      typeof api.runRecursiveActions !== "function"
    )
      throw new Error("Missing recursive action browser API");
    return api.runRecursiveActions();
  });
  expect(result).toEqual({
    matched: 23,
    rejected: 1,
    bounded: 2,
    isolated: false,
  });
});

test("checks levels, lazy functions and shared templates in Chromium", async ({
  page,
}) => {
  await page.goto(origin);
  const result = await page.evaluate(async () => {
    const api: unknown = Reflect.get(globalThis, "portableBrowser");
    if (
      typeof api !== "object" ||
      api === null ||
      !("runFrontendSemantics" in api) ||
      typeof api.runFrontendSemantics !== "function"
    )
      throw new Error("Missing frontend semantics browser API");
    return api.runFrontendSemantics();
  });
  expect(result).toEqual({
    matched: 64,
    rejected: 12,
    modelErrors: 4,
    bounded: 1,
    isolated: false,
  });
});

test("refuses layered calls past the nesting limit and keeps the instance usable in Chromium", async ({
  page,
}) => {
  await page.goto(origin);
  const result = await page.evaluate(async () => {
    const api: unknown = Reflect.get(globalThis, "portableBrowser");
    if (
      typeof api !== "object" ||
      api === null ||
      !("runCallDepth" in api) ||
      typeof api.runCallDepth !== "function"
    )
      throw new Error("Missing call depth browser API");
    return api.runCallDepth();
  });
  expect(result).toEqual({
    accepted: { status: "complete", states: 5, generated: 6, work: 862 },
    refusal: {
      code: "protocol",
      message: "Safety evaluation nesting exceeds 256",
    },
    afterRefusal: { status: "complete", states: 5, generated: 6, work: 432 },
    isolated: false,
  });
});
