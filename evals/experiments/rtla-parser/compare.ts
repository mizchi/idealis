import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { parseConfig, parseModule } from "tla-ts";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { assertOracle } from "../workspace-oracle/node.js";
import { corpus, moduleFields, normalize } from "./corpus.js";
import { syntaxCases } from "./syntax.js";
import { createParserWasm } from "./wasm.js";

export const compare = async () => {
  const inputs = await corpus();
  const requests = inputs.flatMap((input) => [
    { kind: "module", source: input.source },
    { kind: "config", source: input.config },
  ]);
  const native = execFileSync("crates/rtla/target/release/rtla", [], {
    input: `${requests.map((request) => JSON.stringify(request)).join("\n")}\n`,
    maxBuffer: 32 * 1024 * 1024,
  })
    .toString()
    .trim()
    .split("\n")
    .map((line): unknown => JSON.parse(line));
  assert.equal(native.length, requests.length);
  const wasm = await createParserWasm(
    await readFile(
      "crates/rtla/target/wasm32-unknown-unknown/release/rtla.wasm",
    ),
  );
  const records: unknown[] = [];
  for (const [index, input] of inputs.entries()) {
    const module = native[2 * index];
    const config = native[2 * index + 1];
    assertOracle(
      wasm.request(requests[2 * index]),
      module,
      `${input.name}: native/Wasm module`,
    );
    assertOracle(
      wasm.request(requests[2 * index + 1]),
      config,
      `${input.name}: native/Wasm config`,
    );
    assert.ok(
      isRecord(module) && module.kind === "module",
      `${input.name}: ${JSON.stringify(module)}`,
    );
    assert.ok(
      isRecord(config) && config.kind === "config",
      `${input.name}: ${JSON.stringify(config)}`,
    );
    const reference = parseModule(input.source);
    assertOracle(
      moduleFields(module.module),
      moduleFields(reference),
      `${input.name}: module`,
    );
    assertOracle(
      normalize(config.config),
      normalize(parseConfig(input.config)),
      `${input.name}: config`,
    );
    records.push({ name: input.name, module, config });
  }
  const syntaxRecords = [];
  for (const input of syntaxCases) {
    const request = { kind: "module", source: input.source };
    const actual: unknown = JSON.parse(
      execFileSync("crates/rtla/target/release/rtla", [], {
        input: `${JSON.stringify(request)}\n`,
      }).toString(),
    );
    assertOracle(wasm.request(request), actual, input.name);
    assert.ok(isRecord(actual));
    if (input.expected === "module") {
      assert.equal(
        actual.kind,
        "module",
        `${input.name}: ${JSON.stringify(actual)}`,
      );
      assertOracle(
        moduleFields(actual.module),
        moduleFields(parseModule(input.source)),
        input.name,
      );
    } else {
      assert.equal(actual.kind, "error", input.name);
      assert.ok(isRecord(actual.error));
      assert.equal(actual.error.kind, input.expected, input.name);
      if (input.expected === "syntax")
        assert.throws(() => parseModule(input.source), input.name);
      else assert.doesNotThrow(() => parseModule(input.source), input.name);
    }
    syntaxRecords.push({ ...input, actual });
  }
  const limitRecords = [];
  for (const request of [
    {
      kind: "expression",
      source: Array.from({ length: 130 }, () => "1").join("+"),
    },
    { kind: "expression", source: `x${"'".repeat(130)}` },
    { kind: "expression", source: `x${".a".repeat(130)}` },
    { kind: "expression", source: `${"(".repeat(130)}TRUE${")".repeat(130)}` },
    {
      kind: "config",
      source: `CONSTANT N=${"{".repeat(130)}0${"}".repeat(130)}`,
    },
  ]) {
    const actual: unknown = JSON.parse(
      execFileSync("crates/rtla/target/release/rtla", [], {
        input: `${JSON.stringify(request)}\n`,
      }).toString(),
    );
    assertOracle(wasm.request(request), actual);
    assert.ok(
      isRecord(actual) && actual.kind === "error" && isRecord(actual.error),
    );
    assert.equal(actual.error.kind, "unsupported");
    limitRecords.push({ request, actual });
  }
  return {
    inputs,
    records,
    syntaxRecords,
    limitRecords,
    moduleComparisons: inputs.length,
    configComparisons: inputs.length,
    nativeWasmComparisons:
      requests.length + syntaxCases.length + limitRecords.length,
  };
};
