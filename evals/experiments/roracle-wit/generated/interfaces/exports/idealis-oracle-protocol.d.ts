/** @module Interface idealis:oracle/protocol@0.1.0 **/
export function check(request: Batch): string;
export function checkBuffer(data: Uint8Array): string;
export type Observation =
  | ObservationValue
  | ObservationAllowed
  | ObservationUnknown
  | ObservationInvalid
  | ObservationError;
export interface ObservationValue {
  tag: "value";
  val: number;
}
export interface ObservationAllowed {
  tag: "allowed";
  val: Uint32Array;
}
export interface ObservationUnknown {
  tag: "unknown";
  val: string;
}
export interface ObservationInvalid {
  tag: "invalid";
  val: string;
}
export interface ObservationError {
  tag: "error";
  val: string;
}
export interface Input {
  id: string;
  input: number;
  reference: Observation;
  actual: Array<Observation>;
}
export interface Batch {
  version: number;
  group: string;
  reference: string;
  implementations: Array<string>;
  cases: Array<Input>;
  payload: Uint8Array;
}
