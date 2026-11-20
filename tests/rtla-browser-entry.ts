import { createParserWasm } from "../evals/experiments/rtla-parser/wasm.js";
import { isRecord } from "../packages/idealis/src/trace.js";

export const run = async (assetBase = "") => {
  const parser = await createParserWasm(
    new Uint8Array(await (await fetch(`${assetBase}/rtla.wasm`)).arrayBuffer()),
  );
  const source = await (await fetch(`${assetBase}/module.tla`)).text();
  const module = parser.request({ kind: "module", source });
  const unicode = parser.request({
    kind: "expression",
    source: "∀ i ∈ {1, 2}: i ≤ 2",
  });
  const large = parser.request({
    kind: "expression",
    source: "a".repeat(100_000),
  });
  const invalid = parser.request({ kind: "expression", source: "1 = 2 = 3" });
  const limit = parser.request({
    kind: "expression",
    source: `${"(".repeat(130)}TRUE${")".repeat(130)}`,
  });
  const config = parser.request({
    kind: "config",
    source: "CONSTANT Data={a,b}\nSPECIFICATION Spec\nINVARIANT TypeInvariant",
  });
  const label = parser.request({
    kind: "expression",
    source: "a * lab :: b + c",
  });
  const symbolic = parser.request({
    kind: "module",
    source:
      "---- MODULE Symbolic ----\nleft %% right == left\n-. value == value\nvalue ^+ == value\nMain == M!%%(1, 2)\n====",
  });
  const repeated = parser.request({ kind: "module", source });
  if (
    !isRecord(module) ||
    !isRecord(module.module) ||
    !isRecord(unicode) ||
    !isRecord(unicode.expression) ||
    !isRecord(large) ||
    !isRecord(large.expression) ||
    !isRecord(invalid) ||
    !isRecord(invalid.error) ||
    !isRecord(limit) ||
    !isRecord(limit.error) ||
    !isRecord(config) ||
    !isRecord(label) ||
    !isRecord(label.error) ||
    !isRecord(symbolic) ||
    !isRecord(symbolic.module) ||
    !Array.isArray(symbolic.module.definitions)
  )
    throw new Error("Invalid rtla response");
  return {
    name: module.module.name,
    unicode: unicode.expression.kind,
    large:
      typeof large.expression.name === "string"
        ? large.expression.name.length
        : 0,
    invalid: invalid.error.kind,
    label: label.error.kind,
    symbols: symbolic.module.definitions.map((definition) => {
      if (!isRecord(definition)) throw new Error("Invalid symbolic definition");
      return definition.name;
    }),
    limit: limit.error.kind,
    config: config.kind,
    repeat: JSON.stringify(module) === JSON.stringify(repeated),
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
