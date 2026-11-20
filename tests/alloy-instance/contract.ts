import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fixed, graph } from "../../evals/experiments/alloy-instance/corpus.js";
import { processClient } from "../../evals/experiments/smt-subset/clients.js";
import {
  AlloyError,
  createAlloyWasm,
} from "../../packages/idealis/src/alloy.js";

export const registerContract = (
  test: (name: string, run: () => void | Promise<void>) => unknown,
): void => {
  test("public Alloy runtime checks observed graphs and explicit failures", async () => {
    const runtime = await createAlloyWasm(
      await readFile(
        "crates/ralloy/target/wasm32-unknown-unknown/release/ralloy.wasm",
      ),
    );
    const input = {
      source:
        "sig N { edge: set N } fact { no iden & edge } assert Safe { no iden & ^edge } check Safe for 2 but exactly 2 N, 0 Int",
      command: 0,
      maxWork: 100_000,
    };
    const dag = {
      signatures: { N: ["a", "b"] },
      fields: { edge: [["a", "b"]] as const },
    };
    assert.equal(runtime.checkInstance(input, dag).status, "matched");
    const found = runtime.analyze(input);
    assert.equal(found.status, "counterexample");
    assert.ok(found.instance);
    assert.equal(
      runtime.checkInstance(input, found.instance).status,
      "mismatch",
    );
    assert.deepEqual(
      runtime.checkInstance(input, {
        signatures: { N: ["a", "b"] },
        fields: { edge: [["a", "a"]] },
      }).reason,
      "facts",
    );
    assert.equal(
      runtime.checkInstance({ ...input, maxWork: 0 }, dag).status,
      "unknown",
    );
    assert.throws(
      () =>
        runtime.checkInstance(input, {
          signatures: dag.signatures,
          fields: {},
        }),
      AlloyError,
    );
    assert.throws(
      () => runtime.analyze({ ...input, source: "var sig N {} run {}" }),
      AlloyError,
    );
    assert.ok(Object.isFrozen(found.instance.fields));
  });
  test("observed structures agree between public Wasm and native JSONL", async () => {
    const runtime = await createAlloyWasm(
      await readFile(
        "crates/ralloy/target/wasm32-unknown-unknown/release/ralloy.wasm",
      ),
    );
    const native = processClient(
      "crates/ralloy/target/release/ralloy",
      [],
      true,
    );
    try {
      for (const input of [
        ...fixed(),
        ...Array.from({ length: 512 }, (_, mask) =>
          graph(`exhaustive-${mask}`, 3, mask, false),
        ),
      ]) {
        const checked = runtime.checkInstance(input.input, input.instance);
        assert.deepEqual(
          { status: checked.status, reason: checked.reason },
          input.expected,
          input.name,
        );
        assert.deepEqual(
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
      }
    } finally {
      await native.close();
    }
  });
  test("runtime rejects malformed snapshots and invalid budgets", async () => {
    const runtime = await createAlloyWasm(
      await readFile(
        "crates/ralloy/target/wasm32-unknown-unknown/release/ralloy.wasm",
      ),
    );
    const input = fixed()[0];
    assert.ok(input);
    for (const instance of [
      { signatures: { N: ["a", "a"] }, fields: { edge: [] } },
      {
        signatures: { N: ["a", "b"] },
        fields: { edge: [["a", "absent"]] as const },
      },
      {
        signatures: { N: ["a", "b"] },
        fields: {
          edge: [
            ["a", "b"],
            ["a", "b"],
          ] as const,
        },
      },
    ])
      assert.throws(
        () => runtime.checkInstance(input.input, instance),
        (error) => error instanceof AlloyError && error.kind === "protocol",
      );
    assert.throws(
      () => runtime.analyze({ ...input.input, maxWork: Number.NaN }),
      AlloyError,
    );
    assert.throws(
      () => runtime.analyze({ ...input.input, command: -1 }),
      AlloyError,
    );
    const extra = { ...input.instance, extra: true };
    assert.throws(() => runtime.checkInstance(input.input, extra), AlloyError);
  });
};
