import { readFile, writeFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import {
  nativeReplies,
  wasmPath,
} from "../../evals/experiments/sat-foundation/native.js";
import { validate } from "../../evals/experiments/sat-foundation/oracle.js";
import { bundleModule } from "../../scripts/bundle-module.js";
import type { run } from "./browser-entry.js";

test("Varisat solves CNF in Chromium without host imports or isolation", async ({
  page,
}, info) => {
  const bundle = await bundleModule("tests/sat-foundation/browser-entry.ts", {
    globalName: "satEval",
  });
  const wasm = await readFile(wasmPath);
  await page.route("http://sat.test/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    await route.fulfill(
      path === "/bundle.js"
        ? {
            contentType: "application/javascript; charset=utf-8",
            body: bundle.code,
          }
        : path === "/sat.wasm"
          ? { contentType: "application/wasm", body: wasm }
          : {
              contentType: "text/html",
              body: '<!doctype html><script src="/bundle.js"></script>',
            },
    );
  });
  await page.goto("http://sat.test/");
  const result: Awaited<ReturnType<typeof run>> = await page.evaluate(
    async () => {
      const api = Reflect.get(globalThis, "satEval");
      if (typeof api?.run !== "function")
        throw new Error("Missing SAT browser API");
      return api.run();
    },
  );
  expect(result.records).toHaveLength(32);
  expect(result.imports).toEqual([]);
  expect(result.isolated).toBe(false);
  expect(result.budget).toEqual({
    kind: "unsupported",
    reason: "work-budget",
    candidate: "varisat-0.2.2",
  });
  const native = nativeReplies(result.records.map((record) => record.input));
  for (const [index, record] of result.records.entries()) {
    expect(record.statuses).toEqual(record.expected);
    expect(validate(record.input, native[index])).toEqual(record.expected);
  }
  const path = info.outputPath("records.json");
  await writeFile(path, JSON.stringify({ ...result, native }));
  await info.attach("sat-browser-records", {
    path,
    contentType: "application/json",
  });
});
