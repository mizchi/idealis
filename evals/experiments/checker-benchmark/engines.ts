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
import { bounds, type Input, type WarmRoute } from "./protocol.js";

export const wasmPath =
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm";
export const nativePath = "crates/idealis-core/target/release/idealis-core";

export const loadEngines = async (routes: readonly WarmRoute[]) => {
  const core = routes.includes("idealis-wasm")
    ? await createPortableWasm(await readFile(wasmPath))
    : undefined;
  const native = routes.includes("idealis-native") ? startNative() : undefined;
  return {
    run: async (route: WarmRoute, input: Input) => {
      if (route === "tla-ts") {
        const result = await check({
          source: input.source,
          config: input.config,
          version: "1.7.4",
          workers: 1,
          livenessCheck: "final",
        });
        return {
          status: result.status === "success" ? "complete" : result.status,
          states: result.distinctStates,
          generated: result.generatedStates,
          violation: result.status === "success" ? null : result,
        };
      }
      const program = compileTlaSafety(input);
      if (route === "idealis-wasm") {
        assert.ok(core);
        return checkSafety(core, program, bounds);
      }
      assert.ok(native);
      const response = await native.request({
        version: 1,
        request: { kind: "check-safety", program, bounds },
      });
      assert.ok(isRecord(response) && response.kind === "safety");
      return decodeSafetyResult(response.result);
    },
    close: async () => {
      await native?.close();
    },
  };
};
