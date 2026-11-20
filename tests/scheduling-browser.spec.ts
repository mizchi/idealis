import { readFile } from "node:fs/promises";
import { createServer, type Server } from "node:http";
import { expect, test } from "@playwright/test";
import { cases } from "../evals/experiments/rust-safety/cases.js";
import { compileTlaSafety } from "../packages/idealis/src/tla-safety.js";
import { bundleModule } from "../scripts/bundle-module.js";

let server: Server;
let origin: string;
test.beforeAll(async () => {
  const outputs = await Promise.all([
    bundleModule("tests/parallel-safety/browser-test-entry.ts", {
      globalName: "parallelSafety",
    }),
    bundleModule("evals/experiments/parallel-safety/browser-worker.ts"),
    bundleModule("packages/idealis/src/scheduling.ts"),
  ]);
  expect(outputs[2].inputs).toEqual(["packages/idealis/src/scheduling.ts"]);
  const programs = cases
    .filter((input) =>
      ["cycle", "violation", "empty", "depth-cut"].includes(input.name),
    )
    .map((input) => ({ id: input.name, program: compileTlaSafety(input) }));
  const jobs = programs.map((entry) => ({
    id: entry.id,
    program: entry.id,
    estimatedMs: 1,
    memoryBytes: 1024,
    bounds: {
      maxDepth: entry.id === "depth-cut" ? 1 : 100,
      maxStates: 1000,
      maxWork: 100000,
    },
  }));
  const assets = new Map([
    [
      "/bundle.js",
      {
        body: Buffer.from(outputs[0].code),
        type: "text/javascript; charset=utf-8",
      },
    ],
    [
      "/worker.js",
      {
        body: Buffer.from(outputs[1].code),
        type: "text/javascript; charset=utf-8",
      },
    ],
    [
      "/setup.json",
      {
        body: Buffer.from(JSON.stringify({ backend: "wasm", programs, jobs })),
        type: "application/json",
      },
    ],
    [
      "/core.wasm",
      {
        body: await readFile(
          "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
        ),
        type: "application/wasm",
      },
    ],
  ]);
  server = createServer((request, response) => {
    const asset = assets.get(request.url ?? "/");
    response.setHeader("Content-Type", asset?.type ?? "text/html");
    response.end(
      asset?.body ??
        '<!doctype html><title>parallel safety</title><script src="/bundle.js"></script>',
    );
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string")
    throw new Error("Missing browser server");
  origin = `http://127.0.0.1:${address.port}`;
});
test.afterAll(async () => {
  if (server)
    await new Promise<void>((resolve) => server.close(() => resolve()));
});
test("reuses two Wasm workers without cross-origin isolation", async ({
  page,
}) => {
  await page.goto(origin);
  const result = await page.evaluate(async () => {
    const api: unknown = Reflect.get(globalThis, "parallelSafety");
    if (
      typeof api !== "object" ||
      api === null ||
      !("run" in api) ||
      typeof api.run !== "function"
    )
      throw new Error("Missing pool API");
    return api.run();
  });
  expect(result.isolated).toBe(false);
  expect(result.first.executionStatus).toBe("complete");
  expect(result.second.completed).toEqual(result.first.completed);
  expect(
    result.first.completed
      .map(
        (entry: { result: { result: { status: string } } }) =>
          entry.result.result.status,
      )
      .sort(),
  ).toEqual(["bounded", "complete", "empty", "invariant-violation"]);
  expect(result.exhausted.executionStatus).toBe("partial");
  expect(result.exhausted.remaining).toHaveLength(4);
});
