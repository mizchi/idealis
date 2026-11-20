import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  type AlloyInstanceCheck,
  createAlloyWasm,
} from "../../../packages/idealis/src/alloy.js";
import { processClient } from "../smt-subset/clients.js";
import { assertOracle } from "../workspace-oracle/node.js";
import type { Case } from "./corpus.js";
import { nativePath, wasmPath } from "./provenance.js";

export const verify = async (
  inputs: readonly Case[],
  record: (
    input: Case,
    result: AlloyInstanceCheck,
    index: number,
  ) => void | Promise<void>,
) => {
  const runtime = await createAlloyWasm(await readFile(wasmPath));
  const native = processClient(nativePath, [], true);
  try {
    for (const [index, input] of inputs.entries()) {
      const checked = runtime.checkInstance(input.input, input.instance);
      assertOracle(
        { status: checked.status, reason: checked.reason },
        input.expected,
        input.name,
      );
      assertOracle(
        JSON.parse(
          await native.request(
            JSON.stringify({
              version: 1,
              request: {
                kind: "check-instance",
                ...input.input,
                instance: input.instance,
              },
            }),
          ),
        ),
        { kind: "instance-check", result: checked },
      );
      await record(input, checked, index);
    }
    const input = inputs[0];
    assert.ok(input);
    for (const maxWork of [0, 1]) {
      const unknown = runtime.checkInstance(
        { ...input.input, maxWork },
        input.instance,
      );
      assert.equal(unknown.status, "unknown");
      assert.equal(unknown.reason, "work");
    }
  } finally {
    await native.close();
  }
};
