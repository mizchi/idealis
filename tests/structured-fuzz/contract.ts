import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createAudit } from "../../evals/experiments/structured-fuzz/clients.js";
import {
  probes,
  regression,
} from "../../evals/experiments/structured-fuzz/corpus.js";
import { nativePath } from "../../evals/experiments/structured-fuzz/paths.js";
import { equalReply } from "../../evals/experiments/structured-fuzz/wasm.js";

export const register = (
  test: (name: string, run: () => Promise<void> | void) => unknown,
): void => {
  test("saved protocol transcripts agree between native and import-free Wasm", async () => {
    const audit = await createAudit();
    try {
      const records = [];
      for (const input of regression) records.push(await audit.check(input));
      assert.ok(records.some((record) => record.report.phases.shrink));
      assert.ok(records.some((record) => record.report.phases.confirmation));
      assert.ok(records.some((record) => record.report.results.failed));
      for (const bytes of probes(0x3901, 8))
        for (const mode of ["raw", "structured"] as const)
          await audit.check({ mode, bytes: [...bytes] });
    } finally {
      await audit.close();
    }
  });
  test("known session-reset fault and incorrect successful replies are rejected", () => {
    const control = spawnSync(nativePath, ["control"], { encoding: "utf8" });
    assert.notEqual(control.status, 0);
    assert.match(control.stderr, /protocol state differs/);
    assert.equal(
      equalReply({ kind: "effect", effect: 1 }, { kind: "effect", effect: 2 }),
      false,
    );
    assert.equal(equalReply({ kind: "error" }, { kind: "effect" }), false);
  });
};
