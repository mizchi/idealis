import { createRequire } from "node:module";
import { resolve } from "node:path";
import type { OracleRequest } from "../../../packages/idealis/src/generated/roracle.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import {
  decodeOracleReply,
  type OracleClient,
} from "../workspace-oracle/protocol.js";
import { canonicalObject } from "./canonical.js";
import type {
  abiVersion,
  dispatchJson,
  dispatchObject,
} from "./generated/index.js";

interface NativeBinding {
  readonly abiVersion: typeof abiVersion;
  readonly dispatchJson: typeof dispatchJson;
  readonly dispatchObject: typeof dispatchObject;
}

export const nativePath =
  "evals/experiments/roracle-native/generated/roracle-native.node";

export const loadNativeBinding = (path = nativePath): NativeBinding => {
  const input: unknown = createRequire(import.meta.url)(resolve(path));
  if (
    !isRecord(input) ||
    typeof input.abiVersion !== "function" ||
    input.abiVersion() !== 1 ||
    typeof input.dispatchJson !== "function" ||
    typeof input.dispatchObject !== "function"
  )
    throw new Error("Invalid native oracle ABI");
  const nativeVersion = input.abiVersion;
  const nativeJson = input.dispatchJson;
  const nativeObject = input.dispatchObject;
  return {
    abiVersion: () => {
      const version: unknown = nativeVersion();
      if (typeof version !== "number")
        throw new Error("Invalid native ABI version");
      return version;
    },
    dispatchJson: (value) => {
      const result: unknown = nativeJson(value);
      if (typeof result !== "string")
        throw new Error("Invalid native JSON reply");
      return result;
    },
    dispatchObject: (value) => {
      const result: unknown = nativeObject(value);
      if (typeof result !== "string")
        throw new Error("Invalid native object reply");
      return result;
    },
  };
};

export const createNativeOracle = (
  mode: "native-json" | "native-object",
  path = nativePath,
): OracleClient => {
  const binding = loadNativeBinding(path);
  return {
    request: (input: OracleRequest) => {
      if (mode === "native-object") {
        canonicalObject(input);
        return decodeOracleReply(binding.dispatchObject(input));
      }
      return decodeOracleReply(binding.dispatchJson(JSON.stringify(input)));
    },
  };
};
