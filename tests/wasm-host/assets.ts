import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { cases } from "../../evals/experiments/collections-temporal/cases.js";
import { wasmPath } from "../../evals/experiments/sat-foundation/native.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";

export const binaries: Readonly<Record<string, string>> = {
  "/core.wasm":
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  "/rtla.wasm": "crates/rtla/target/wasm32-unknown-unknown/release/rtla.wasm",
  "/runtime.wasm":
    "evals/experiments/rust-probe/target/wasm32-unknown-unknown/release/idealis_runtime_probe.wasm",
  "/subset.wasm":
    "crates/rusmt/target/wasm32-unknown-unknown/release/rusmt.wasm",
  "/ralloy.wasm":
    "crates/ralloy/target/wasm32-unknown-unknown/release/ralloy.wasm",
  "/testing.wasm":
    "crates/idealis-connect/target/wasm32-unknown-unknown/release/idealis_connect.wasm",
  "/sat.wasm": wasmPath,
  "/wire.wasm":
    "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
  "/typed-oracle.wasm":
    "evals/experiments/oracle-comparison/rust/target/wasm32-unknown-unknown/release/oracle_comparison.wasm",
};

const files: Readonly<Record<string, string>> = {
  ...binaries,
  "/module.tla": "crates/rtla/tests/fixtures/Channel.tla",
  "/finite.json": "crates/rtla-compiler/tests/finite-support.json",
  "/modules.json": "crates/rtla-compiler/tests/modules-support.json",
  "/operators.json": "crates/rtla-compiler/tests/operators-support.json",
  "/completion.json": "crates/rtla-compiler/tests/frontend-completion.json",
  "/recursive-actions.json":
    "crates/rtla-compiler/tests/recursive-actions.json",
  "/frontend-semantics.json":
    "crates/rtla-compiler/tests/frontend-semantics.json",
};

export const withAssets = async <Result>(
  run: (origin: string) => Promise<Result>,
  extra: Readonly<Record<string, string>> = {},
): Promise<Result> => {
  const generated: Readonly<Record<string, string>> = {
    "/safety.json": JSON.stringify(
      compileTlaSafety({
        source:
          "---- MODULE HostSafety ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x+1\nSafe == x<3\n====",
        config: "INIT Init\nNEXT Next\nINVARIANT Safe",
      }),
    ),
    "/complex.json": JSON.stringify(
      [
        "set-fault",
        "function-updates",
        "sequence-queue",
        "multiple-eventualities",
        "SF-intermittent",
        "temporal-node-cut",
      ].map((name) => {
        const input = cases.find((input) => input.name === name);
        assert.ok(input, name);
        return {
          program: compileTlaSafety(input),
          bounds: input.bounds,
          expected: input.expected,
        };
      }),
    ),
    ...extra,
  };
  const server = createServer(async (request, response) => {
    const path = new URL(request.url ?? "/", "http://wasm.local").pathname;
    try {
      response.setHeader(
        "Content-Type",
        path.endsWith(".wasm") ? "application/wasm" : "application/json",
      );
      if (Object.hasOwn(generated, path)) response.end(generated[path]);
      else if (Object.hasOwn(files, path))
        response.end(await readFile(files[path] ?? ""));
      else {
        response.statusCode = 404;
        response.end();
      }
    } catch (error) {
      response.statusCode = 500;
      response.end(error instanceof Error ? error.message : String(error));
    }
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  try {
    return await run(`http://127.0.0.1:${address.port}`);
  } finally {
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
};
