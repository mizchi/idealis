import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import {
  decodeOracleReply,
  type OracleClient,
} from "../workspace-oracle/protocol.js";
import { encodeBatch, encodeBuffer } from "./encode.js";
import type { instantiate } from "./generated/oracle.js";
import { witClient } from "./wit-client.js";

export const createTypedClient = async (
  mode: "native-buffer" | "wit-record" | "wit-buffer",
  directory = "evals/experiments/roracle-wit/generated",
): Promise<OracleClient> => {
  if (mode === "native-buffer") {
    const binding: unknown = createRequire(import.meta.url)(
      resolve(directory, "roracle-typed.node"),
    );
    if (!isRecord(binding) || typeof binding.checkBuffer !== "function")
      throw new Error("Invalid typed native binding");
    const check = binding.checkBuffer;
    return {
      request: (input) => {
        const data = encodeBuffer(encodeBatch(input));
        if (data.buffer instanceof SharedArrayBuffer)
          throw new Error("Shared typed buffer is unsupported");
        const result: unknown = check(
          Buffer.from(data.buffer, data.byteOffset, data.byteLength),
        );
        if (typeof result !== "string")
          throw new Error("Invalid typed native report");
        return decodeOracleReply(result);
      },
    };
  }
  const component: { readonly instantiate: typeof instantiate } = await import(
    pathToFileURL(resolve(directory, "oracle.js")).href
  );
  const root = component.instantiate(
    (path) => new WebAssembly.Module(readFileSync(resolve(directory, path))),
    {},
  );
  return witClient(root, mode);
};
