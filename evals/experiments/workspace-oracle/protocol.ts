import type {
  CheckReport,
  JsonValue,
  OracleRequest,
  Status,
} from "../../../packages/idealis/src/generated/roracle.js";

export interface OracleReport {
  readonly status: Status;
  readonly report: CheckReport<JsonValue, JsonValue>;
}
export interface OracleClient {
  readonly request: (input: OracleRequest) => OracleReport;
}

const object = (value: JsonValue | undefined, names: readonly string[]) => {
  if (
    !value ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.keys(value).length !== names.length ||
    names.some((name) => !Object.hasOwn(value, name))
  )
    throw new Error("Invalid oracle report fields");
  return value;
};
const text = (value: JsonValue | undefined) => {
  if (typeof value !== "string") throw new Error("Invalid oracle report text");
  return value;
};
const checkStatus = (value: JsonValue | undefined): void => {
  if (
    value !== "matched" &&
    value !== "mismatch" &&
    value !== "unknown" &&
    value !== "invalid" &&
    value !== "error"
  )
    throw new Error("Invalid oracle report status");
};
const checkObservation = (input: JsonValue | undefined): void => {
  if (!input || typeof input !== "object" || Array.isArray(input))
    throw new Error("Invalid oracle observation");
  switch (input.kind) {
    case "value": {
      const value = object(input, ["kind", "value"]).value;
      if (value === undefined) throw new Error("Missing oracle value");
      return;
    }
    case "allowed": {
      const values = object(input, ["kind", "values"]).values;
      if (!Array.isArray(values))
        throw new Error("Invalid allowed observations");
      return;
    }
    case "unknown":
    case "invalid":
      object(input, ["kind", "reason"]);
      text(input.reason);
      return;
    case "error":
      object(input, ["kind", "message"]);
      text(input.message);
      return;
    default:
      throw new Error("Invalid oracle observation kind");
  }
};

function checkReport(
  input: JsonValue | undefined,
): asserts input is CheckReport<JsonValue, JsonValue> {
  const report = object(input, [
    "version",
    "group",
    "reference",
    "implementations",
    "status",
    "cases",
  ]);
  if (
    report.version !== 1 ||
    !Array.isArray(report.implementations) ||
    !Array.isArray(report.cases)
  )
    throw new Error("Invalid oracle report");
  if (!report.cases.length || !report.implementations.length)
    throw new Error("Empty oracle report");
  for (const id of report.implementations) text(id);
  text(report.group);
  text(report.reference);
  checkStatus(report.status);
  for (const entry of report.cases) {
    const row = object(entry, ["case", "reference", "comparisons"]);
    const saved = object(row.case, ["id", "input"]);
    if (saved.input === undefined || !Array.isArray(row.comparisons))
      throw new Error("Invalid oracle case");
    if (row.comparisons.length !== report.implementations.length)
      throw new Error("Oracle comparison dimensions differ");
    text(saved.id);
    checkObservation(row.reference);
    for (const entry of row.comparisons) {
      const comparison = object(entry, ["id", "status", "actual"]);
      text(comparison.id);
      checkStatus(comparison.status);
      checkObservation(comparison.actual);
    }
  }
}

export const decodeOracleReply = (json: string): OracleReport => {
  const input: JsonValue = JSON.parse(json);
  if (
    input &&
    typeof input === "object" &&
    !Array.isArray(input) &&
    input.kind === "error"
  ) {
    const failed = object(input, ["kind", "message"]);
    throw new Error(`Oracle batch failed: ${text(failed.message)}`);
  }
  const response = object(input, ["kind", "report"]);
  if (response.kind !== "checked") throw new Error("Invalid oracle reply kind");
  const report = response.report;
  checkReport(report);
  return { status: report.status, report };
};
