import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { bundleModule } from "../../scripts/bundle-module.js";

test("SMT-LIB scripts and shared budgets run without host imports in Chromium", async ({
  page,
}) => {
  const bundle = await bundleModule("tests/smt-scripts/browser-entry.ts", {
    globalName: "smtScripts",
  });
  const wasm = await readFile(
    "crates/rusmt/target/wasm32-unknown-unknown/release/rusmt.wasm",
  );
  await page.route("http://smt-scripts.test/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/bundle.js")
      await route.fulfill({
        contentType: "application/javascript; charset=utf-8",
        body: bundle.code,
      });
    else if (path === "/subset.wasm")
      await route.fulfill({ contentType: "application/wasm", body: wasm });
    else
      await route.fulfill({
        contentType: "text/html",
        body: '<!doctype html><script src="/bundle.js"></script>',
      });
  });
  await page.goto("http://smt-scripts.test/");
  const result = await page.evaluate(async () => {
    const api = Reflect.get(globalThis, "smtScripts");
    if (typeof api?.run !== "function") throw new Error("Missing API");
    return api.run();
  });
  expect(result).toEqual({
    scripts: 7,
    checks: 48,
    bounded: ["sat", "unknown", "unknown"],
    empty: 0,
    error: "error",
    preserved: "9",
    isolated: false,
  });
});
