import type {
  JsonValue,
  Observation,
  OracleRequest,
} from "../../../packages/idealis/src/generated/roracle.js";
import { snapshot } from "../workspace-oracle/snapshot.js";

export interface Fixture {
  readonly name: string;
  readonly iterations: number;
  readonly input: OracleRequest;
  readonly expected: "matched" | "mismatch" | "unknown";
}

const batch = (
  value: JsonValue,
  count: number,
  failure: "matched" | "mismatch" | "unknown" = "matched",
): OracleRequest => ({
  version: 1,
  group: "native-transport",
  reference: "reference",
  implementations: ["implementation"],
  cases: Array.from({ length: count }, (_, index) => ({
    id: `case-${index}`,
    input: { index },
    reference:
      failure === "unknown"
        ? { kind: "unknown", reason: "work" }
        : { kind: "value", value },
    actual: [
      { kind: "value", value: failure === "mismatch" ? "changed" : value },
    ],
  })),
});

export const fixtures: readonly Fixture[] = [
  { name: "single", iterations: 1000, input: batch(7, 1), expected: "matched" },
  {
    name: "batch-32",
    iterations: 100,
    input: batch({ rows: [1, true, null, "stable"] }, 32),
    expected: "matched",
  },
  {
    name: "nested-256",
    iterations: 100,
    input: batch(
      Array.from({ length: 256 }, (_, index) => ({
        index,
        value: [true, index, null],
      })),
      1,
    ),
    expected: "matched",
  },
  {
    name: "unicode",
    iterations: 1000,
    input: batch({ text: "日本語の観測😀", nested: ["雪", "λ", "é"] }, 1),
    expected: "matched",
  },
  {
    name: "mismatch",
    iterations: 1000,
    input: batch({ value: 7 }, 1, "mismatch"),
    expected: "mismatch",
  },
  {
    name: "unknown",
    iterations: 1000,
    input: batch(7, 1, "unknown"),
    expected: "unknown",
  },
];

export const observationMatrix: readonly Observation<JsonValue>[] = [
  { kind: "value", value: null },
  { kind: "value", value: 7 },
  { kind: "value", value: { nested: [1, true, "雪"] } },
  { kind: "allowed", values: [null, 7] },
  { kind: "allowed", values: [] },
  { kind: "unknown", reason: "work" },
  { kind: "invalid", reason: "scope" },
  { kind: "error", message: "checker" },
];

export const requestFor = (
  reference: Observation<JsonValue>,
  actual: Observation<JsonValue>,
): OracleRequest => ({
  version: 1,
  group: "contract",
  reference: "oracle",
  implementations: ["sut"],
  cases: [
    {
      id: "one",
      input: snapshot({
        negativeZero: -0,
        bigint: 3n,
        missing: undefined,
        nan: NaN,
      }),
      reference,
      actual: [actual],
    },
  ],
});
