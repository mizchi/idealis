import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { bundleModule } from "../../scripts/bundle-module.js";

test("expanded arithmetic and contexts run without host imports in Chromium", async ({
  page,
}) => {
  const bundle = await bundleModule("tests/smt-expansion/browser-entry.ts", {
    globalName: "smtExpansion",
  });
  const wasm = await readFile(
    "crates/rusmt/target/wasm32-unknown-unknown/release/rusmt.wasm",
  );
  await page.route("http://smt-expansion.test/**", async (route) => {
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
  await page.goto("http://smt-expansion.test/");
  const result = await page.evaluate(async () => {
    const api = Reflect.get(globalThis, "smtExpansion");
    if (typeof api?.run !== "function") throw new Error("Missing API");
    return api.run();
  });
  expect(result).toEqual({
    checked: 36,
    positive: "3",
    negative: "4",
    error: "error",
    afterError: "3",
    unknown: "unknown",
    restored: "9",
    isolated: false,
  });
});
