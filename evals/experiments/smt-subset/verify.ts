import assert from "node:assert/strict";
import { checkConformance } from "../../../packages/idealis/src/conformance.js";
import { createTtlDriver, type TtlAction, ttlModel } from "../harness/ttl.js";
import { assertOracle } from "../workspace-oracle/node.js";
import { createNative, createZ3, maxWork } from "./clients.js";
import type { Input } from "./corpus.js";
import { benchmarks, controls } from "./corpus.js";
import type { createWasm } from "./wasm.js";
import { solution } from "./wire.js";

export const verify = async (
  inputs: readonly Input[],
  core: Awaited<ReturnType<typeof createWasm>>,
  onRecord: (record: unknown) => void,
) => {
  const native = createNative();
  const z3 = await createZ3();
  let sat = 0;
  let unsat = 0;
  try {
    for (const input of inputs) {
      const wasm = solution(
        core.request({ kind: "solve", source: input.source, maxWork }),
      );
      const actual = solution(
        await native.request({ kind: "solve", source: input.source, maxWork }),
      );
      const reference = await z3.solve(input);
      onRecord({ input, wasm, native: actual, reference });
      assertOracle(actual, wasm, input.name);
      assert.equal(wasm.status, input.expected, input.name);
      assert.equal(reference.status, input.expected, input.name);
      if (wasm.status === "sat") {
        assertOracle(
          Object.keys(wasm.values).sort(),
          [...input.integers].sort(),
        );
        assertOracle(
          Object.keys(wasm.booleans).sort(),
          [...input.booleans].sort(),
        );
        await z3.validate(input, wasm);
        sat++;
      } else unsat++;
      const handle = core.request({ kind: "prepare", source: input.source });
      assert.equal(handle.kind, "prepared");
      if (handle.kind !== "prepared") throw new Error("Expected prepared");
      assertOracle(
        solution(
          core.request({ kind: "check", handle: handle.handle, maxWork }),
        ),
        wasm,
      );
      assert.equal(
        core.request({ kind: "release", handle: handle.handle }).kind,
        "released",
      );
      assert.equal(
        core.request({ kind: "check", handle: handle.handle, maxWork }).kind,
        "error",
      );
    }
    for (const source of controls) {
      const wasm = core.request({ kind: "solve", source, maxWork });
      const actual = await native.request({ kind: "solve", source, maxWork });
      assert.equal(wasm.kind, "error", source);
      assertOracle(actual, wasm);
      onRecord({ control: source, reply: wasm });
    }
    const input = benchmarks()[0];
    assert.ok(input);
    const cut = solution(
      core.request({ kind: "solve", source: input.source, maxWork: 0 }),
    );
    assert.equal(cut.status, "unknown");
    assertOracle(cut.values, {});
    onRecord({ budget: 0, reply: cut });
    const model = solution(
      core.request({ kind: "solve", source: input.source, maxWork }),
    );
    await z3.validate(input, model);
    const actions: readonly TtlAction[] = [
      { type: "put", ttl: Number(model.values.ttl) },
      { type: "advance", amount: Number(model.values.elapsed) },
      { type: "read" },
    ];
    const broken = await checkConformance(
      ttlModel,
      createTtlDriver(true),
      actions,
    );
    const fixed = await checkConformance(
      ttlModel,
      createTtlDriver(false),
      actions,
    );
    assert.equal(broken.status, "mismatch");
    assert.equal(fixed.status, "matched");
    onRecord({
      sut: "known TTL boundary fault",
      model,
      actions,
      broken,
      fixed,
    });
    return {
      sat,
      unsat,
      controls: controls.length,
      budgetUnknown: true,
      sut: { broken: broken.status, fixed: fixed.status },
      z3Version: z3.version,
    };
  } finally {
    await Promise.all([native.close(), z3.close()]);
  }
};
