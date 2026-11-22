import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { bundleModule } from "../../scripts/bundle-module.js";

test("cached SMT checks run opt-in without host imports in Chromium", async ({
  page,
}) => {
  const bundle = await bundleModule("tests/smt-cache/browser-entry.ts", {
    globalName: "smtCache",
  });
  const wasm = await readFile(
    "crates/rusmt/target/wasm32-unknown-unknown/release/rusmt.wasm",
  );
  await page.route("http://smt-cache.test/**", async (route) => {
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
  await page.goto("http://smt-cache.test/");
  const result = await page.evaluate(async () => {
    const api = Reflect.get(globalThis, "smtCache");
    if (typeof api?.run !== "function") throw new Error("Missing API");
    return api.run();
  });
  expect(result).toEqual({
    cold: { hits: 1, misses: 2, writes: 2, invalidated: false },
    warm: { hits: 3, misses: 0, writes: 0, invalidated: false },
    values: ["3", "4", "3"],
    work: [0, 0, 0],
    disabled: ["unknown", "unknown", "unknown"],
    isolated: false,
  });
});
