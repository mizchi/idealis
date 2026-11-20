import type {
  JsonValue,
  Observation,
  OracleRequest,
  Status,
} from "../../../packages/idealis/src/generated/roracle.js";
import type { dispatchObject } from "./generated/index.js";

const valid: OracleRequest = {
  version: 1,
  group: "types",
  reference: "expected",
  implementations: ["actual"],
  cases: [
    {
      id: "one",
      input: null,
      reference: { kind: "value", value: 7 },
      actual: [{ kind: "value", value: 7 }],
    },
  ],
};
const nativeInput: Parameters<typeof dispatchObject>[0] = valid;
void nativeInput;
// @ts-expect-error The Rust status enum does not include success.
const wrongStatus: Status = "success";
// @ts-expect-error Observation tags come from the Rust enum.
const wrongObservation: Observation<JsonValue> = { kind: "matched" };
// @ts-expect-error Extra request fields are not part of the generated contract.
const wrongRequest: OracleRequest = { ...valid, unexpected: true };
void wrongStatus;
void wrongObservation;
void wrongRequest;

const wrongNative: Parameters<typeof dispatchObject>[0] = {
  ...valid,
  // @ts-expect-error Native binding arguments must retain the generated contract.
  version: "1",
};
void wrongNative;
