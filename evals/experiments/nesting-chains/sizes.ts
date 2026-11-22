import type { PortableCommand } from "../../../packages/idealis/src/contracts.js";
import {
  type Limits,
  mebibytes,
  runNative,
  runTypeScript,
  runWasm,
} from "../../../tests/tlc-differential/operator-sharing-support.js";

export const sizeCounts = [4096, 100_000] as const;
export const sizeBounds = { maxDepth: 8, maxStates: 100, maxWork: 100_000 };
export const sizeLimits: Limits = {
  milliseconds: 5_000,
  bytes: mebibytes(1024),
};

export type SizeSurface = "ts" | "wasm" | "native";
export const sizeSurfaces: readonly SizeSurface[] = ["ts", "wasm", "native"];

export const sizeInput = (terms: number) => ({
  source: `---- MODULE Sizes ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext ==\n${Array.from({ length: terms }, () => "  \\/ (x' = 0)").join("\n")}\nInv == x >= 0\n====\n`,
  config: "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
  irVersion: 2 as const,
});

export const measureSize = async (surface: SizeSurface, terms: number) => {
  const input = sizeInput(terms);
  if (surface === "ts") return runTypeScript(input, sizeBounds, sizeLimits);
  const request: PortableCommand["request"] = {
    kind: "check-tla",
    ...input,
    bounds: sizeBounds,
  };
  return surface === "wasm"
    ? runWasm(request, sizeLimits)
    : runNative(request, sizeLimits);
};
