import assert from "node:assert/strict";
import { assertOracle } from "../workspace-oracle/node.js";
import { createNative, maxWork } from "./clients.js";
import { type Case, controls, source } from "./corpus.js";
import type { createWasm } from "./wasm.js";
import { analysis } from "./wire.js";

export const verify = async (
  inputs: readonly Case[],
  wasm: Awaited<ReturnType<typeof createWasm>>,
  onRecord: (record: unknown) => void,
) => {
  const native = createNative();
  try {
    for (const input of inputs) {
      const request = {
        kind: "analyze" as const,
        source: source(input),
        command: 0,
        maxWork,
      };
      const result = analysis(await native.request(request));
      assertOracle(
        wasm.request(request),
        { kind: "analysis", result },
        input.name,
      );
      assert.equal(result.status, input.expected, input.name);
      assert.ok(result.status !== "unknown", input.name);
      onRecord({ input, result });
    }
    for (const source of controls) {
      const request = { kind: "analyze" as const, source, command: 0, maxWork };
      const result = await native.request(request);
      assert.equal(result.kind, "error", source);
      assertOracle(wasm.request(request), result);
      onRecord({ control: source, result });
    }
    const input = inputs[0];
    assert.ok(input);
    const unknown = analysis(
      wasm.request({
        kind: "analyze",
        source: source(input),
        command: 0,
        maxWork: 0,
      }),
    );
    assert.equal(unknown.status, "unknown");
    assert.equal(unknown.instance, null);
    const badCommand = wasm.request({
      kind: "analyze",
      source: source(input),
      command: 99,
      maxWork,
    });
    assert.equal(badCommand.kind, "error");
    return {
      cases: inputs.length,
      controls: controls.length,
      budgetUnknown: true,
    };
  } finally {
    await native.close();
  }
};
