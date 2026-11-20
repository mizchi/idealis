import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import type {
  PortableCommand,
  SafetyProgram,
} from "../../../packages/idealis/src/contracts.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { startNative } from "../usable/native.js";
import { bounds, type Input } from "./outcome.js";

export type Compile = (input: Input) => Promise<SafetyProgram>;
export type Check = (program: SafetyProgram) => Promise<unknown>;

export const nativeFrontend = (
  request: (command: PortableCommand) => Promise<unknown>,
): { readonly compile: Compile; readonly check: Check } => ({
  compile: async (input) => {
    const reply = await request({
      version: 1,
      request: { kind: "compile-tla", ...input },
    });
    assert.ok(isRecord(reply));
    if (reply.kind === "compiled") return reply.program as SafetyProgram;
    throw new Error(String(reply.message));
  },
  check: (program) =>
    request({ version: 1, request: { kind: "check-safety", program, bounds } }),
});

export const openBuild = async (
  binary: string,
  wasm: string,
  distribution: string,
) => {
  const native = startNative(binary);
  const core = await createPortableWasm(await readFile(wasm));
  const module: unknown = await import(
    pathToFileURL(`${distribution}/packages/idealis/src/tla-safety.js`).href
  );
  assert.ok(isRecord(module) && typeof module.compileTlaSafety === "function");
  const typescript = module.compileTlaSafety as (input: Input) => SafetyProgram;
  const rust = nativeFrontend((command) => native.request(command));
  return {
    ts: {
      compile: async (input: Input) => typescript(input),
      check: rust.check,
    },
    rust,
    wasm: {
      compile: async (input: Input): Promise<SafetyProgram> => {
        const reply = core.request({
          version: 1,
          request: { kind: "compile-tla", ...input },
        });
        assert.equal(reply.kind, "compiled");
        return reply.program;
      },
    },
    close: () => native.close(),
  };
};
