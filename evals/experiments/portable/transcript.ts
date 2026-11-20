import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import type {
  PortableCommand,
  PortableCore,
  PortableOutput,
} from "../../../packages/idealis/src/contracts.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable.js";
import { assertOracle } from "../workspace-oracle/node.js";

export const createNativeComparison = async (bytes: Uint8Array) => {
  const start = performance.now();
  const wasm = await createPortableWasm(bytes);
  const startupMs = performance.now() - start;
  const commands: PortableCommand[] = [];
  const outputs: PortableOutput[] = [];
  const core: PortableCore = {
    request: (command) => {
      commands.push(structuredClone(command));
      const output = wasm.request(command);
      outputs.push(output);
      return output;
    },
  };
  return {
    core,
    startupMs,
    verify: () => {
      const start = performance.now();
      const native = execFileSync(
        "crates/idealis-core/target/release/idealis-core",
        {
          input: commands.map((command) => JSON.stringify(command)).join("\n"),
          maxBuffer: 64 * 1024 * 1024,
        },
      )
        .toString()
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line));
      assert.equal(native.length, outputs.length, "Native reply count differs");
      for (const [index, output] of outputs.entries())
        assertOracle(
          native[index],
          output,
          `Native/Wasm reply ${index} differs`,
        );
      return { replies: native.length, elapsedMs: performance.now() - start };
    },
  };
};
