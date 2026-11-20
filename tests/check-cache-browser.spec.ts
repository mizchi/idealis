import { readFile } from "node:fs/promises";
import { createServer, type Server } from "node:http";
import { expect, test } from "@playwright/test";
import { names } from "../evals/experiments/quint-generated/cases.js";
import { createCheckCacheHttpHandler } from "../packages/idealis/src/check-cache-node.js";
import { createMemoryCheckCacheStore } from "../packages/idealis/src/check-cache.js";
import { bundleModule } from "../scripts/bundle-module.js";

let server: Server;
let origin: string;
const models = await Promise.all(
  names.map(async (name) => ({
    name,
    source: await readFile(
      `fixtures/quint-generated/generated/${name}.tla`,
      "utf8",
    ),
  })),
);
test.beforeAll(async () => {
  const bundle = await bundleModule("tests/check-cache-browser-entry.ts", {
    globalName: "cacheSuite",
  });
  expect(
    bundle.inputs.some(
      (path) => path.includes("check-cache-node") || path.startsWith("node:"),
    ),
  ).toBe(false);
  const wasm = await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  );
  const handler = createCheckCacheHttpHandler(createMemoryCheckCacheStore());
  server = createServer((request, response) => {
    if (request.url?.startsWith("/cache/")) {
      request.url = request.url.slice("/cache".length);
      void handler(request, response);
    } else if (request.url === "/bundle.js") {
      response.setHeader("Content-Type", "text/javascript");
      response.end(bundle.code);
    } else if (request.url === "/core.wasm") {
      response.setHeader("Content-Type", "application/wasm");
      response.end(wasm);
    } else {
      response.setHeader("Content-Type", "text/html");
      response.end(
        '<!doctype html><title>verified cache</title><script src="/bundle.js"></script>',
      );
    }
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string")
    throw new Error("Missing cache test server");
  origin = `http://127.0.0.1:${address.port}`;
});
test.afterAll(async () => {
  if (server)
    await new Promise<void>((resolve) => server.close(() => resolve()));
});
test("two fresh browser clients reuse genuine Quint checks through HTTP", async ({
  page,
}) => {
  await page.goto(origin);
  const invoke = (runId: string) =>
    page.evaluate(
      async ({ endpoint, inputs, id }) => {
        const api: unknown = Reflect.get(globalThis, "cacheSuite");
        if (
          typeof api !== "object" ||
          api === null ||
          !("run" in api) ||
          typeof api.run !== "function"
        )
          throw new Error("Missing cache browser API");
        return api.run(endpoint, inputs, id);
      },
      { endpoint: `${origin}/cache`, inputs: models, id: runId },
    );
  const cold = await invoke("cold");
  await page.reload();
  const warm = await invoke("warm");
  expect(cold.calls).toBe(6);
  expect(warm.calls).toBe(0);
  expect(cold.rows.map((row: { kind: string }) => row.kind)).toEqual(
    Array(6).fill("executed"),
  );
  expect(warm.rows.map((row: { kind: string }) => row.kind)).toEqual(
    Array(6).fill("cached"),
  );
  expect(warm.rows.map((row: { states: number }) => row.states)).toEqual([
    4, 1, 1, 3, 3, 1,
  ]);
  expect(warm.rows.map((row: { outcome: unknown }) => row.outcome)).toEqual(
    cold.rows.map((row: { outcome: unknown }) => row.outcome),
  );
});

test("fresh Wasm instances reuse state expansions and recheck changed Quint properties", async ({
  page,
}) => {
  await page.goto(origin);
  const cold = await page.evaluate(async (inputs) => {
    const api: unknown = Reflect.get(globalThis, "cacheSuite");
    if (
      typeof api !== "object" ||
      api === null ||
      !("runExpansions" in api) ||
      typeof api.runExpansions !== "function"
    )
      throw new Error("Missing expansion API");
    return api.runExpansions(inputs);
  }, models);
  await page.reload();
  const warm = await page.evaluate(
    async ({ inputs, saved }) => {
      const api: unknown = Reflect.get(globalThis, "cacheSuite");
      if (
        typeof api !== "object" ||
        api === null ||
        !("runExpansions" in api) ||
        typeof api.runExpansions !== "function"
      )
        throw new Error("Missing expansion API");
      return api.runExpansions(inputs, saved);
    },
    {
      inputs: models,
      saved: cold.map(
        (row: { normal: { cache: unknown } }) => row.normal.cache,
      ),
    },
  );
  expect(
    warm.map(
      (row: { normal: { stats: { hits: number; misses: number } } }) =>
        row.normal.stats.hits,
    ),
  ).toEqual([4, 1, 1, 3, 3, 1]);
  expect(
    warm.every(
      (row: { normal: { stats: { misses: number } } }) =>
        row.normal.stats.misses === 0,
    ),
  ).toBe(true);
  expect(
    warm.every(
      (row: { changed: { result: { status: string } } }) =>
        row.changed.result.status === "invariant-violation",
    ),
  ).toBe(true);
});
