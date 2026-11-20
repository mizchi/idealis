import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { check } from "tla-ts";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import {
  checkSafety,
  decodeSafetyResult,
} from "../../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { startNative } from "../usable/native.js";
import { type BenchmarkCase, bounds } from "./cases.js";

const backend = process.argv[2];
const path = process.argv[3];
const workers = !process.argv[4] ? undefined : Number(process.argv[4]);
const binary = process.argv[5];
assert.ok(
  (backend === "native" || backend === "wasm" || backend === "tla-ts") && path,
);
assert.ok(
  workers === undefined ||
    (backend === "native" && Number.isSafeInteger(workers) && workers > 0),
);
const input: BenchmarkCase = JSON.parse(await readFile(path, "utf8"));
if (backend === "tla-ts") {
  const started = performance.now();
  const result = await check({
    source: input.source,
    config: input.config,
    version: "1.7.4",
    workers: 1,
    livenessCheck: "final",
  });
  console.log(
    JSON.stringify(
      { apiMs: performance.now() - started, result },
      (_key, value: unknown) =>
        typeof value === "bigint"
          ? value.toString()
          : value instanceof Map
            ? { $map: [...value] }
            : value instanceof Set
              ? { $set: [...value] }
              : value,
    ),
  );
} else {
  const frontendStart = performance.now();
  const program = compileTlaSafety(input);
  const frontendMs = performance.now() - frontendStart;
  const start = performance.now();
  const core =
    backend === "wasm"
      ? await createPortableWasm(
          await readFile(
            binary ??
              "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
          ),
        )
      : undefined;
  const native = backend === "native" ? startNative(binary) : undefined;
  if (native)
    await native.request({
      version: 1,
      request: { kind: "shrink-candidates", length: 0 },
    });
  const engineStartupMs = performance.now() - start;
  try {
    const apiStart = performance.now();
    const response = core
      ? checkSafety(core, program, bounds)
      : await native?.request({
          version: 1,
          request:
            workers === undefined
              ? { kind: "check-safety", program, bounds }
              : { kind: "check-safety-parallel", program, bounds, workers },
        });
    const result = core
      ? response
      : (() => {
          assert.ok(
            isRecord(response) && response.kind === "safety",
            JSON.stringify(response),
          );
          return decodeSafetyResult(response.result);
        })();
    console.log(
      JSON.stringify({
        frontendMs,
        engineStartupMs,
        apiMs: performance.now() - apiStart,
        result,
      }),
    );
  } finally {
    await native?.close();
  }
}
