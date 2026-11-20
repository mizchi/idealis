import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

test("typed differential checks, recorded replay and host clock run in native and browser Wasm", async ({
  page,
}) => {
  const path = "evals/experiments/oracle-comparison/rust/target";
  const native = Number(
    execFileSync(`${path}/release/oracle-comparison`, ["--probe"]).toString(),
  );
  const bytes = await readFile(
    `${path}/wasm32-unknown-unknown/release/oracle_comparison.wasm`,
  );
  const module = await WebAssembly.compile(bytes);
  expect(WebAssembly.Module.imports(module)).toEqual([]);
  await page.route("http://oracle.test/**", async (route) => {
    await route.fulfill(
      new URL(route.request().url()).pathname === "/oracle.wasm"
        ? { contentType: "application/wasm", body: bytes }
        : {
            contentType: "text/html",
            body: "<!doctype html><title>Oracle probe</title>",
          },
    );
  });
  await page.goto("http://oracle.test");
  const actual = await page.evaluate(async () => {
    const wasm = await WebAssembly.instantiateStreaming(
      fetch("/oracle.wasm"),
      {},
    );
    const probe = wasm.instance.exports.oracle_probe;
    if (typeof probe !== "function") throw new Error("Missing probe");
    return { result: probe(), isolated: crossOriginIsolated };
  });
  expect(actual).toEqual({ result: native, isolated: false });
  expect(native).toBe(51);
});
