import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { bundleModule } from "../../scripts/bundle-module.js";

test("workspace snapshots use the actual oracle JSON ABI in Chromium", async ({
  page,
}) => {
  const bundle = await bundleModule("tests/workspace-oracle/browser-entry.ts", {
    globalName: "workspaceOracle",
  });
  const bytes = await readFile(
    "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
  );
  expect(WebAssembly.Module.imports(await WebAssembly.compile(bytes))).toEqual(
    [],
  );
  await page.route("http://workspace-oracle.test/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    await route.fulfill(
      path === "/wire.wasm"
        ? { contentType: "application/wasm", body: bytes }
        : path === "/bundle.js"
          ? {
              contentType: "application/javascript; charset=utf-8",
              body: bundle.code,
            }
          : {
              contentType: "text/html",
              body: '<!doctype html><script src="/bundle.js"></script>',
            },
    );
  });
  await page.goto("http://workspace-oracle.test");
  const actual = await page.evaluate("workspaceOracle.run()");
  expect(actual).toEqual({
    matched: "matched",
    mismatch: "mismatch",
    isolated: false,
  });
});
