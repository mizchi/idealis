import assert from "node:assert/strict";
import { PortableCoreError } from "../packages/idealis/src/portable-value.js";
import { decodeSafetyCoverageReport } from "../packages/idealis/src/safety-coverage.js";
import { decodeSafetyResult } from "../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../packages/idealis/src/tla-safety.js";

export const registerSafetyContract = (
  test: (name: string, run: () => void | Promise<void>) => unknown,
): void => {
  const source =
    "---- MODULE Counter ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nSafe == x < 3\n====";
  const config = "INIT Init\nNEXT Next\nINVARIANT Safe";
  test("coverage validates counts, exact fields and conservation of totals", () => {
    const row = {
      name: "Init",
      attempts: 1,
      enabled: 1,
      generated: 1,
      distinct: 1,
      work: 4,
    };
    const report = {
      result: {
        status: "complete",
        reason: null,
        states: 1,
        generated: 2,
        depth: 0,
        work: 9,
        violation: null,
      },
      coverage: {
        initial: [row],
        next: [{ ...row, name: "Next", distinct: 0, work: 3 }],
        unattributedWork: 2,
      },
    };
    assert.deepEqual(decodeSafetyCoverageReport(report), report);
    for (const coverage of [
      { ...report.coverage, extra: 1 },
      { ...report.coverage, unattributedWork: -1 },
      { ...report.coverage, unattributedWork: 1.5 },
      { ...report.coverage, unattributedWork: 0 },
      { ...report.coverage, initial: null },
      ...[
        { ...row, extra: true },
        { ...row, name: 1 },
        { ...row, attempts: 0 },
        { ...row, enabled: 2 },
        { ...row, enabled: 0 },
        { ...row, distinct: 2 },
        { ...row, work: 0xffff_ffff + 1 },
        { ...row, work: 0 },
        { ...row, generated: 0 },
        null,
      ].map((inner) => ({ ...report.coverage, initial: [inner] })),
    ])
      assert.throws(
        () => decodeSafetyCoverageReport({ ...report, coverage }),
        PortableCoreError,
      );
    assert.throws(
      () => decodeSafetyCoverageReport({ ...report, extra: 1 }),
      PortableCoreError,
    );
    assert.throws(
      () => decodeSafetyCoverageReport({ coverage: report.coverage }),
      PortableCoreError,
    );
  });
  test("safety compilation produces a portable program without enumerating reachable states", () => {
    const program = compileTlaSafety({ source, config });
    assert.deepEqual(program.variables, ["x"]);
    assert.equal(program.format, "modelt.safety-program");
    assert.equal(program.next.length, 1);
    assert.ok(JSON.stringify(program).length < 1000);
    assert.equal(program.invariants[0]?.name, "Safe");
  });
  test("safety compilation rejects unsupported configuration instead of ignoring it", () => {
    for (const suffix of [
      "CONSTRAINT Safe",
      "ACTION_CONSTRAINT Next",
      "VIEW x",
      "SYMMETRY Safe",
      "POSTCONDITION Safe",
    ])
      assert.throws(() =>
        compileTlaSafety({ source, config: `${config}\n${suffix}` }),
      );
    assert.throws(() =>
      compileTlaSafety({ source, config: "SPECIFICATION Spec" }),
    );
    assert.equal(
      compileTlaSafety({ source, config: `${config}\nPROPERTY Safe` })
        .initialProperties?.[0]?.name,
      "Safe",
    );
    assert.equal(
      compileTlaSafety({
        source: source.replace("Naturals", "Sequences"),
        config,
      }).next.length,
      1,
    );
    assert.equal(
      compileTlaSafety({
        source: source.replace("x' = x + 1", "x' = <<x>>"),
        config,
      }).next[0]?.steps[0]?.kind,
      "assign",
    );
  });
  test("safety compilation rejects missing arithmetic imports and constant NEXT", () => {
    assert.throws(() =>
      compileTlaSafety({
        source: source.replace("EXTENDS Naturals\n", ""),
        config,
      }),
    );
    assert.throws(() =>
      compileTlaSafety({
        source: source.replace("x' = x + 1", "FALSE"),
        config,
      }),
    );
  });
  test("safety compilation rejects unbound, recursive and conflicting symbols", () => {
    assert.throws(() =>
      compileTlaSafety({
        source: source.replace("x + 1", "missing + 1"),
        config,
      }),
    );
    assert.throws(() =>
      compileTlaSafety({ source: source.replace("x + 1", "Next"), config }),
    );
    assert.throws(() =>
      compileTlaSafety({
        source: source.replace("VARIABLE x", "VARIABLE x\nF(x) == x"),
        config,
      }),
    );
    assert.throws(() =>
      compileTlaSafety({
        source: source.replace("Safe == x < 3", "Safe == x' < 3"),
        config,
      }),
    );
  });
  test("safety result validates lasso boundaries and preserves compound snapshots", () => {
    const result = {
      status: "liveness-violation",
      reason: null,
      states: 1,
      generated: 2,
      depth: 0,
      work: 10,
      violation: {
        name: "P",
        trace: [
          {
            action: null,
            values: [{ type: "list", value: [{ type: "int", value: "1" }] }],
          },
        ],
        loopStart: 0,
        loopAction: "<stuttering>",
      },
    };
    assert.deepEqual(decodeSafetyResult(result), result);
    for (const violation of [
      { ...result.violation, loopStart: 1 },
      { ...result.violation, loopStart: -1 },
      { ...result.violation, loopStart: "0" },
      { ...result.violation, loopAction: null },
      { name: "P", trace: result.violation.trace },
    ])
      assert.throws(() => decodeSafetyResult({ ...result, violation }));
    assert.throws(() =>
      decodeSafetyResult({ ...result, status: "invariant-violation" }),
    );
  });
};
