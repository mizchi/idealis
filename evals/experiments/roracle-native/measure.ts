import assert from "node:assert/strict";
import type { OracleRequest } from "../../../packages/idealis/src/generated/roracle.js";
import type {
  OracleClient,
  OracleReport,
} from "../workspace-oracle/protocol.js";
import { createOracle } from "../workspace-oracle/wasm.js";
import { createNativeOracle } from "./client.js";

export const modes = ["wasm", "native-json", "native-object"] as const;
export type Mode = (typeof modes)[number];

export interface SavedFixture {
  readonly name: string;
  readonly iterations: number;
  readonly input: OracleRequest;
  readonly expected: OracleReport;
}
export interface Sample {
  readonly fixture: string;
  readonly mode: Mode;
  readonly sample: number;
  readonly calls: number;
  readonly totalNs: number;
}
export interface ColdSample {
  readonly mode: Mode;
  readonly sample: number;
  readonly initAndFirstUs: number;
  readonly processMs: number;
}

export const createClient = (
  mode: Mode,
  wasm: Uint8Array,
  native: string,
): OracleClient =>
  mode === "wasm" ? createOracle(wasm) : createNativeOracle(mode, native);

const median = (values: readonly number[]) => {
  assert.ok(values.length);
  assert.ok(values.every((value) => Number.isFinite(value) && value > 0));
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.floor(sorted.length / 2);
  return sorted.length % 2
    ? (sorted[index] ?? 0)
    : ((sorted[index - 1] ?? 0) + (sorted[index] ?? 0)) / 2;
};

export const summarize = (
  fixtures: readonly SavedFixture[],
  samples: readonly Sample[],
  cold: readonly ColdSample[],
) => {
  assert.equal(samples.length, fixtures.length * modes.length * 30);
  assert.equal(cold.length, modes.length * 10);
  const rows = fixtures.map((fixture) => {
    const timings = modes.map((mode) => {
      const selected = samples.filter(
        (sample) => sample.fixture === fixture.name && sample.mode === mode,
      );
      assert.equal(selected.length, 30);
      assert.deepEqual(
        selected.map((sample) => sample.sample),
        Array.from({ length: 30 }, (_, index) => index),
      );
      assert.ok(
        selected.every((sample) => sample.calls === fixture.iterations),
      );
      return (
        median(selected.map((sample) => sample.totalNs / sample.calls)) / 1000
      );
    });
    const wasm = timings[0];
    const nativeJson = timings[1];
    const nativeObject = timings[2];
    assert.ok(wasm && nativeJson && nativeObject);
    return {
      fixture: fixture.name,
      wasmUs: wasm,
      nativeJsonUs: nativeJson,
      nativeObjectUs: nativeObject,
      jsonSpeedup: wasm / nativeJson,
      objectSpeedup: wasm / nativeObject,
    };
  });
  return {
    rows,
    candidates: (["native-json", "native-object"] as const).map((mode) => {
      const ratios = rows.map((row) =>
        mode === "native-json" ? row.jsonSpeedup : row.objectSpeedup,
      );
      const geometricMean = Math.exp(
        ratios.reduce((total, ratio) => total + Math.log(ratio), 0) /
          ratios.length,
      );
      const worstRatio = Math.min(...ratios);
      return {
        mode,
        geometricMean,
        worstRatio,
        eligible: geometricMean >= 1.2 && worstRatio >= 1 / 1.1,
      };
    }),
    cold: modes.map((mode) => {
      const selected = cold.filter((sample) => sample.mode === mode);
      assert.equal(selected.length, 10);
      assert.deepEqual(
        selected.map((sample) => sample.sample),
        Array.from({ length: 10 }, (_, index) => index),
      );
      return {
        mode,
        initAndFirstUs: median(selected.map((sample) => sample.initAndFirstUs)),
        processMs: median(selected.map((sample) => sample.processMs)),
      };
    }),
  };
};
