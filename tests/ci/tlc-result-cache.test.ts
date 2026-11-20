import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { nativePath, wasmPath } from "./tlc-build.js";
import { createResultCache } from "./tlc-result-cache.js";

test("only identical complete checks are reused after reopening a cache with the same checker version", async (t) => {
  const directory = mkdtempSync(join(tmpdir(), "tlc-results-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const request = {
    source: "A",
    config: "Safe",
    modules: { B: "b" },
    bounds: 2,
  };
  const open = (build: string) =>
    createResultCache(
      directory,
      build,
      (r: { status: string; n: bigint }) => r.status === "complete",
    );
  let calls = 0;
  const execute = () => ({ status: "complete", n: BigInt(++calls) });
  const first = open("native-v1");
  assert.equal(first.runSync(request, execute).n, 1n);
  assert.equal(
    (await open("native-v1").run(request, async () => execute())).n,
    1n,
  );
  const file = readdirSync(directory)[0]!;
  writeFileSync(join(directory, file), "partial");
  assert.equal(first.runSync(request, execute).n, 2n);
  for (const change of [
    { source: "B" },
    { config: "Other" },
    { modules: { B: "changed" } },
    { bounds: 3 },
  ])
    first.runSync({ ...request, ...change }, execute);
  assert.equal(open("native-v2").runSync(request, execute).n, 7n);
  assert.equal(open("wasm-v1").runSync(request, execute).n, 8n);
  for (const status of ["bounded", "empty", "violation", "error"]) {
    const incomplete = () => ({ status, n: BigInt(++calls) });
    assert.notEqual(
      first.runSync(status, incomplete).n,
      first.runSync(status, incomplete).n,
    );
  }
  await assert.rejects(
    first.run("failure", async () => {
      throw new Error("infrastructure");
    }),
  );
  assert.equal(first.runSync("failure", execute).status, "complete");
});

test("Actions cache namespace changes on a version bump or a rebuild at the same version", (t) => {
  const directory = mkdtempSync(join(tmpdir(), "tlc-build-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const put = (path: string, text: string) => {
    mkdirSync(join(directory, dirname(path)), { recursive: true });
    writeFileSync(join(directory, path), text);
  };
  for (const path of [
    nativePath,
    wasmPath,
    "pnpm-lock.yaml",
    "dist/evals/experiments/usable/native.js",
    "dist/packages/idealis/src/host.js",
    "dist/tests/ci/cache.js",
  ])
    put(path, "initial");
  put("crates/rtlc/Cargo.toml", 'version = "1.0"');
  const fingerprint = () =>
    execFileSync(
      process.execPath,
      [
        "--input-type=module",
        "-e",
        `import { tlcBuild } from ${JSON.stringify(import.meta.resolve("./tlc-build.js"))}; console.log(tlcBuild());`,
      ],
      { cwd: directory, encoding: "utf8" },
    ).trim();
  const initial = fingerprint();
  assert.match(initial, /^rtlc-1\.0-tla-ts-/);
  put("unrelated.tla", "changed model");
  assert.equal(fingerprint(), initial);
  put(nativePath, "new native binary");
  const rebuilt = fingerprint();
  assert.notEqual(rebuilt, initial);
  put(wasmPath, "new Wasm binary");
  const wasm = fingerprint();
  assert.notEqual(wasm, rebuilt);
  put("crates/rtlc/Cargo.toml", 'version = "2.0"');
  assert.notEqual(fingerprint(), wasm);
});
