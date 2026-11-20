import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { processClient } from "../smt-subset/clients.js";
import { nativePath, wasmPath } from "./paths.js";
import { createWasm, equalReply } from "./wasm.js";

export interface Input {
  readonly mode: "raw" | "structured";
  readonly bytes: readonly number[];
}
export interface Report {
  readonly commands: number;
  readonly engineCalls: number;
  readonly errors: number;
  readonly bounded: boolean;
  readonly phases: Readonly<Record<string, number>>;
  readonly results: Readonly<Record<string, number>>;
  readonly trace: readonly {
    readonly request: readonly number[];
    readonly response: unknown;
  }[];
}
export interface AuditRecord {
  readonly input: Input;
  readonly report: Report;
  readonly wasm: readonly unknown[];
}
export const createAudit = async () => {
  const client = processClient(nativePath, [], true);
  const wasm = await createWasm(await readFile(wasmPath));
  return {
    close: client.close,
    check: async (input: Input): Promise<AuditRecord> => {
      const report: Report = JSON.parse(
        await client.request(JSON.stringify(input)),
      );
      assert.ok(report.engineCalls <= 256);
      wasm.request(
        new TextEncoder().encode('{"version":1,"request":{"kind":"cancel"}}'),
      );
      const replies = report.trace.map((entry) => {
        const response = wasm.request(Uint8Array.from(entry.request));
        assert.ok(
          equalReply(entry.response, response),
          `native/Wasm reply differs: ${JSON.stringify({ entry, response })}`,
        );
        return response;
      });
      return { input, report, wasm: replies };
    },
  };
};
