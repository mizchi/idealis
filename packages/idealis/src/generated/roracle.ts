// Generated from roracle Rust contracts. Run just oracle-types.
export type JsonValue =
  | number
  | string
  | boolean
  | Array<JsonValue>
  | { [key in string]: JsonValue }
  | null;
export type Status = "matched" | "mismatch" | "invalid" | "unknown" | "error";
export type Observation<T> =
  | { kind: "value"; value: T }
  | { kind: "allowed"; values: Array<T> }
  | { kind: "unknown"; reason: string }
  | { kind: "invalid"; reason: string }
  | { kind: "error"; message: string };
export type WireObservation = Observation<JsonValue>;
export type Case<I> = { id: string; input: I };
export type Comparison<O> = {
  id: string;
  status: Status;
  actual: Observation<O>;
};
export type CaseReport<I, O> = {
  case: Case<I>;
  reference: Observation<O>;
  comparisons: Array<Comparison<O>>;
};
export type CheckReport<I, O> = {
  version: number;
  group: string;
  reference: string;
  implementations: Array<string>;
  status: Status;
  cases: Array<CaseReport<I, O>>;
};
export type OracleInput = {
  id: string;
  input: JsonValue;
  reference: WireObservation;
  actual: Array<WireObservation>;
};
export type OracleRequest = {
  version: number;
  group: string;
  reference: string;
  implementations: Array<string>;
  cases: Array<OracleInput>;
};
export type OracleResponse =
  | { kind: "checked"; report: CheckReport<JsonValue, JsonValue> }
  | { kind: "error"; message: string };
