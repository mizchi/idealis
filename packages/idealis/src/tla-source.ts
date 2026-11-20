import type {
  PortableCore,
  SafetyBounds,
  SafetyProgram,
  SafetyResult,
  TlaSourceInput,
} from "./contracts.js";
import { PortableCoreError } from "./portable-value.js";

export const compileTlaSource = (
  core: PortableCore,
  input: TlaSourceInput,
): SafetyProgram => {
  const result = core.request({
    version: 1,
    request: {
      kind: "compile-tla",
      source: input.source,
      config: input.config,
      ...(input.modules ? { modules: input.modules } : {}),
      ...(input.irVersion === undefined ? {} : { irVersion: input.irVersion }),
      ...(input.syntaxVersion === undefined
        ? {}
        : { syntaxVersion: input.syntaxVersion }),
    },
  });
  if (result.kind !== "compiled")
    throw new PortableCoreError("protocol", "Expected compiled TLA program");
  return result.program;
};
export const checkTlaSource = (
  core: PortableCore,
  input: TlaSourceInput,
  bounds: SafetyBounds,
): SafetyResult => {
  const result = core.request({
    version: 1,
    request: {
      kind: "check-tla",
      source: input.source,
      config: input.config,
      ...(input.modules ? { modules: input.modules } : {}),
      ...(input.irVersion === undefined ? {} : { irVersion: input.irVersion }),
      ...(input.syntaxVersion === undefined
        ? {}
        : { syntaxVersion: input.syntaxVersion }),
      bounds,
    },
  });
  if (result.kind !== "safety")
    throw new PortableCoreError("protocol", "Expected TLA safety result");
  return result.result;
};
