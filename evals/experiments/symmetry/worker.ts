import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { decodeSafetyProgram } from "../../../packages/idealis/src/safety-program.js";
import { decodeSafetyResult } from "../../../packages/idealis/src/safety.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { startNative } from "../usable/native.js";
import { bounds, type Input } from "./cases.js";

const [path, workerArgument, binary] = process.argv.slice(2);
assert.ok(path && workerArgument && binary);
const workers = Number(workerArgument);
const input: Input = JSON.parse(await readFile(path, "utf8"));
const native = startNative(binary);
try {
  const compileStarted = performance.now();
  const compiled = await native.request({
    version: 1,
    request: {
      kind: "compile-tla",
      source: input.source,
      config: input.config,
    },
  });
  assert.ok(
    isRecord(compiled) && compiled.kind === "compiled",
    JSON.stringify(compiled),
  );
  const program = decodeSafetyProgram(compiled.program);
  const compileAndStartupMs = performance.now() - compileStarted;
  const checkStarted = performance.now();
  const checked = await native.request({
    version: 1,
    request: { kind: "check-safety-parallel", program, bounds, workers },
  });
  assert.ok(
    isRecord(checked) && checked.kind === "safety",
    JSON.stringify(checked),
  );
  console.log(
    JSON.stringify({
      result: decodeSafetyResult(checked.result),
      compileAndStartupMs,
      checkMs: performance.now() - checkStarted,
    }),
  );
} finally {
  await native.close();
}
