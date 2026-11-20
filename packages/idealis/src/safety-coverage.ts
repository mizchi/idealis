import type {
  PortableCore,
  SafetyBounds,
  SafetyCoverageReport,
  SafetyProgram,
} from "./contracts.js";
import { PortableCoreError } from "./portable-value.js";
import { decodeSafetyResult } from "./safety.js";
import { isRecord } from "./trace.js";

export const decodeSafetyCoverageReport = (
  input: unknown,
): SafetyCoverageReport => {
  const fields = (input: unknown, keys: readonly string[]) => {
    if (
      !isRecord(input) ||
      Object.keys(input).sort().join() !== [...keys].sort().join()
    )
      throw new PortableCoreError("protocol", "Invalid safety coverage fields");
    return input;
  };
  const count = (input: unknown) => {
    if (
      typeof input !== "number" ||
      !Number.isSafeInteger(input) ||
      input < 0 ||
      input > 0xffff_ffff
    )
      throw new PortableCoreError("protocol", "Invalid safety coverage count");
    return input;
  };
  const rows = (input: unknown) => {
    if (!Array.isArray(input))
      throw new PortableCoreError("protocol", "Invalid safety coverage rows");
    return Array.from(input, (inner) => {
      const value = fields(inner, [
        "name",
        "attempts",
        "enabled",
        "generated",
        "distinct",
        "work",
      ]);
      if (typeof value.name !== "string")
        throw new PortableCoreError("protocol", "Invalid safety coverage name");
      const row = {
        name: value.name,
        attempts: count(value.attempts),
        enabled: count(value.enabled),
        generated: count(value.generated),
        distinct: count(value.distinct),
        work: count(value.work),
      };
      if (
        row.enabled > row.attempts ||
        row.enabled > row.generated ||
        row.distinct > row.generated ||
        (row.enabled === 0 && row.generated > 0) ||
        (row.attempts === 0 && row.work > 0)
      )
        throw new PortableCoreError(
          "protocol",
          "Inconsistent safety coverage row",
        );
      return row;
    });
  };
  const report = fields(input, ["result", "coverage"]);
  const result = decodeSafetyResult(report.result);
  const value = fields(report.coverage, [
    "initial",
    "next",
    "unattributedWork",
  ]);
  const coverage = {
    initial: rows(value.initial),
    next: rows(value.next),
    unattributedWork: count(value.unattributedWork),
  };
  const actions = [...coverage.initial, ...coverage.next];
  if (
    actions.reduce((total, row) => total + row.generated, 0) !==
      result.generated ||
    actions.reduce((total, row) => total + row.distinct, 0) !== result.states ||
    actions.reduce(
      (total, row) => total + row.work,
      coverage.unattributedWork,
    ) !== result.work
  )
    throw new PortableCoreError(
      "protocol",
      "Inconsistent safety coverage totals",
    );
  return { result, coverage };
};

export const checkSafetyCoverage = (
  core: PortableCore,
  program: SafetyProgram,
  bounds: SafetyBounds,
): SafetyCoverageReport => {
  const output = core.request({
    version: 1,
    request: { kind: "check-safety-coverage", program, bounds },
  });
  if (output.kind !== "safety-coverage")
    throw new PortableCoreError(
      "protocol",
      "Expected a safety coverage report",
    );
  return output.report;
};
