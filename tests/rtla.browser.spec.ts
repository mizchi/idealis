import { readFile } from "node:fs/promises";
import { createServer, type Server } from "node:http";
import { expect, test } from "@playwright/test";
import { bundleModule } from "../scripts/bundle-module.js";

let server: Server | undefined;
let origin: string;
test.beforeAll(async () => {
  const bundle = await bundleModule("tests/rtla-browser-entry.ts", {
    globalName: "parserBrowser",
  });
  const wasm = await readFile(
    "crates/rtla/target/wasm32-unknown-unknown/release/rtla.wasm",
  );
  const source = await readFile("crates/rtla/tests/fixtures/Channel.tla");
  server = createServer((request, response) => {
    response.setHeader(
      "Content-Type",
      request.url === "/rtla.wasm"
        ? "application/wasm"
        : request.url === "/bundle.js"
          ? "text/javascript; charset=utf-8"
          : request.url === "/module.tla"
            ? "text/plain"
            : "text/html",
    );
    response.end(
      request.url === "/rtla.wasm"
        ? wasm
        : request.url === "/bundle.js"
          ? Buffer.from(bundle.code)
          : request.url === "/module.tla"
            ? source
            : '<!doctype html><title>rtla</title><script src="/bundle.js"></script>',
    );
  });
  await new Promise<void>((resolve) => server?.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string")
    throw new Error("Missing parser server address");
  origin = `http://127.0.0.1:${address.port}`;
});
test.afterAll(async () => {
  if (server)
    await new Promise<void>((resolve, reject) =>
      server?.close((error) => (error ? reject(error) : resolve())),
    );
});
test("rtla parses TLA/config in Chromium and reuses memory after a syntax error", async ({
  page,
}) => {
  await page.goto(origin);
  const result = await page.evaluate(async () => {
    const api: unknown = Reflect.get(globalThis, "parserBrowser");
    if (
      typeof api !== "object" ||
      api === null ||
      !("run" in api) ||
      typeof api.run !== "function"
    )
      throw new Error("Missing parser API");
    return api.run();
  });
  expect(result).toEqual({
    name: "Channel",
    unicode: "Forall",
    large: 100_000,
    invalid: "syntax",
    label: "syntax",
    symbols: ["%%", "-.", "^+", "Main"],
    limit: "unsupported",
    config: "config",
    repeat: true,
    isolated: false,
  });
});
