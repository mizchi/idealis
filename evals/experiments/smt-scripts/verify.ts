import assert from "node:assert/strict";
import {
  createNative,
  createZ3,
  maxWork,
  processClient,
} from "../smt-subset/clients.js";
import type { createWasm } from "../smt-subset/wasm.js";
import {
  decode,
  type Reply,
  type Request,
  solution,
} from "../smt-subset/wire.js";
import { assertOracle } from "../workspace-oracle/node.js";
import { controls, type ScriptInput } from "./corpus.js";

export const verify = async (
  inputs: readonly ScriptInput[],
  core: Awaited<ReturnType<typeof createWasm>>,
  onRecord: (record: unknown) => void,
  baselinePath?: string,
  baselineCore?: Awaited<ReturnType<typeof createWasm>>,
) => {
  const native = createNative();
  const z3 = await createZ3();
  const baseline = baselinePath
    ? processClient(baselinePath, [], true)
    : undefined;
  let sat = 0;
  let unsat = 0;
  let baselineAccepted = 0;
  const request = async (command: Request): Promise<Reply> => {
    const wasm = core.request(command);
    const actual = await native.request(command);
    onRecord({ request: command, native: actual, wasm });
    assertOracle(actual, wasm);
    return wasm;
  };
  try {
    for (const input of inputs) {
      const reply = await request({
        kind: "run-script",
        source: input.source,
        maxWork,
      });
      assert.equal(reply.kind, "script-results", input.name);
      if (reply.kind !== "script-results")
        throw new Error(JSON.stringify(reply));
      assert.equal(reply.results.length, input.snapshots.length);
      const output = await z3.script(input.source);
      const statuses = output.trim().split(/\s+/);
      onRecord({
        name: input.name,
        steps: input.steps,
        source: input.source,
        z3ScriptOutput: output,
      });
      assertOracle(
        statuses,
        input.snapshots.map((value) => value.expected),
      );
      for (const [index, result] of reply.results.entries()) {
        const snapshot = input.snapshots[index];
        assert.ok(snapshot);
        const reference = await z3.solve(snapshot);
        onRecord({ snapshot, actual: result, reference });
        assert.equal(result.status, snapshot.expected, snapshot.name);
        assert.equal(result.status, reference.status, snapshot.name);
        if (result.status === "sat") {
          assertOracle(Object.keys(result.values), snapshot.integers);
          assertOracle(Object.keys(result.booleans), snapshot.booleans);
          await z3.validate(snapshot, result);
          sat++;
        } else unsat++;
      }
      if (baseline) {
        const command = {
          kind: "solve",
          source: input.source,
          maxWork,
        } satisfies Request;
        const previous = decode(
          JSON.parse(
            await baseline.request(
              JSON.stringify({ version: 1, request: command }),
            ),
          ),
        );
        assertOracle(previous, baselineCore?.request(command));
        onRecord({ baseline: input.name, reply: previous });
        if (previous.kind === "solution") {
          assert.equal(input.snapshots.length, 1);
          assert.equal(previous.result.status, input.snapshots[0]?.expected);
          if (previous.result.status === "sat") {
            assert.ok(input.snapshots[0]);
            await z3.validate(input.snapshots[0], previous.result);
          }
          baselineAccepted++;
        } else assert.equal(previous.kind, "error");
      }
    }
    const handle = await request({
      kind: "prepare",
      source: "(declare-const x Int)(assert (= x 9))",
    });
    assert.equal(handle.kind, "prepared");
    if (handle.kind !== "prepared") throw new Error("Expected handle");
    for (const source of controls) {
      for (const budget of [0, maxWork])
        assert.equal(
          (await request({ kind: "run-script", source, maxWork: budget })).kind,
          "error",
          source,
        );
      assert.equal(
        solution(
          await request({ kind: "check", handle: handle.handle, maxWork }),
        ).values.x,
        "9",
      );
    }
    assert.equal(
      (await request({ kind: "release", handle: handle.handle })).kind,
      "released",
    );
    for (const source of ["", "(declare-const x Int)(assert (> x 0))"])
      assertOracle(await request({ kind: "run-script", source, maxWork: 0 }), {
        kind: "script-results",
        results: [],
      });
    const source =
      "(declare-const x Int)(assert (= x 7))(check-sat)(check-sat)(check-sat)";
    const full = await request({ kind: "run-script", source, maxWork });
    assert.equal(full.kind, "script-results");
    if (full.kind !== "script-results") throw new Error("Expected results");
    const first = full.results[0];
    assert.ok(first);
    const bounded = await request({
      kind: "run-script",
      source,
      maxWork: first.stats.work,
    });
    assert.equal(bounded.kind, "script-results");
    if (bounded.kind !== "script-results") throw new Error("Expected results");
    assertOracle(
      bounded.results.map((value) => value.status),
      ["sat", "unknown", "unknown"],
    );
    assert.equal(
      bounded.results.reduce((sum, result) => sum + result.stats.work, 0),
      first.stats.work,
    );
    const zero = await request({ kind: "run-script", source, maxWork: 0 });
    assert.equal(zero.kind, "script-results");
    if (zero.kind !== "script-results") throw new Error("Expected results");
    assertOracle(
      zero.results.map((value) => value.status),
      ["unknown", "unknown", "unknown"],
    );
    assertOracle(await request({ kind: "run-script", source, maxWork }), full);
    assert.ok(sat > 0 && unsat > 0);
    return {
      scripts: inputs.length,
      sat,
      unsat,
      checks: sat + unsat,
      controls: controls.length,
      sharedBudget: true,
      emptyResults: true,
      baselineAccepted: baseline ? baselineAccepted : null,
      z3Version: z3.version,
    };
  } finally {
    await Promise.all([native.close(), z3.close(), baseline?.close()]);
  }
};
