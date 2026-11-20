import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { bundleModule } from "../../scripts/bundle-module.js";

test("public testing runtime generates, shrinks and replays real counter failures", async ({
  page,
}) => {
  const bundle = await bundleModule("tests/testing-core/browser-entry.ts", {
    globalName: "testing",
  });
  const wasm = await readFile(
    "crates/idealis-connect/target/wasm32-unknown-unknown/release/idealis_connect.wasm",
  );
  await page.route("http://testing.test/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    await route.fulfill(
      path === "/bundle.js"
        ? {
            contentType: "application/javascript; charset=utf-8",
            body: bundle.code,
          }
        : path === "/testing.wasm"
          ? { contentType: "application/wasm", body: wasm }
          : {
              contentType: "text/html",
              body: '<!doctype html><script src="/bundle.js"></script>',
            },
    );
  });
  await page.goto("http://testing.test/");
  expect(
    await page.evaluate(async () => {
      const api = Reflect.get(globalThis, "testing");
      if (typeof api?.run !== "function")
        throw new Error("Missing testing API");
      return api.run();
    }),
  ).toEqual({
    normal: "passed",
    broken: "failed",
    actions: ["add", "crash", "restart"],
    shrink: "minimal",
    replay: "failed",
    fixed: "passed",
    unknown: "inconclusive",
    isolated: false,
  });
});
