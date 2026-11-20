import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { createNativeOracle } from "../roracle-native/client.js";
import type { OracleClient } from "../workspace-oracle/protocol.js";
import { createOracle } from "../workspace-oracle/wasm.js";
import { createTypedClient } from "./node.js";

export const modes = [
  "json-wasm",
  "native-json",
  "native-object",
  "wit-record",
  "wit-buffer",
  "native-buffer",
] as const;
export type Mode = (typeof modes)[number];
export const createEngine = async (
  mode: Mode,
  assets: string,
): Promise<OracleClient> => {
  if (mode === "json-wasm")
    return createOracle(await readFile(resolve(assets, "roracle.wasm")));
  if (mode === "native-json" || mode === "native-object")
    return createNativeOracle(mode, resolve(assets, "roracle-native.node"));
  return createTypedClient(mode, resolve(assets, "generated"));
};
