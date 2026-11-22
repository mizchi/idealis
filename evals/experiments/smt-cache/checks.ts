import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import type { ScriptInput } from "../smt-scripts/corpus.js";
import { generated, handCases } from "../smt-scripts/corpus.js";
import { nativePath, wasmPath } from "../smt-subset/paths.js";
import { createWasm } from "../smt-subset/wasm.js";
import type {
  CheckCache,
  Reply,
  Request,
  Solution,
} from "../smt-subset/wire.js";
import { decode } from "../smt-subset/wire.js";

/** Runs a request batch in one engine instance and returns every reply. */
export type Engine = (requests: readonly Request[]) => Reply[];
export const nativeEngine =
  (path: string): Engine =>
  (requests) =>
    execFileSync(path, {
      input: requests
        .map((request) => JSON.stringify({ version: 1, request }))
        .join("\n"),
      encoding: "utf8",
      maxBuffer: 1 << 28,
    })
      .trim()
      .split("\n")
      .map((line) => decode(JSON.parse(line)));
export const answer = (solution: Solution) => {
  const { status, values, booleans, reason } = solution;
  return { status, values, booleans, reason };
};

/**
 * Drives the same cached request sequence through every engine. Cold runs
 * must equal plain run-script, warm runs must return the same answers with
 * zero work, and disabled runs must ignore the snapshot. Engines must agree.
 */
export const verifyScriptCache = (
  inputs: readonly ScriptInput[],
  engines: Readonly<Record<string, Engine>>,
  buildDigest: string,
  onRecord: (record: unknown) => void,
) => {
  const transcripts: Record<string, unknown[]> = {};
  let hits = 0;
  for (const [name, engine] of Object.entries(engines)) {
    const plain = engine(
      inputs.map((input) => ({
        kind: "run-script",
        source: input.source,
        maxWork: 2_000_000,
      })),
    );
    let cache: CheckCache | undefined;
    const transcript: unknown[] = [];
    for (const [index, input] of inputs.entries()) {
      const expected = plain[index];
      if (expected?.kind !== "script-results") throw new Error(input.name);
      const run = (
        maxWork: number,
        enabled: boolean,
      ): Extract<Reply, { kind: "incremental-script-results" }> => {
        const [reply] = engine([
          {
            kind: "run-script-incremental",
            source: input.source,
            maxWork,
            enabled,
            buildDigest,
            maxEntries: 4096,
            ...(cache ? { cache } : {}),
          },
        ]);
        if (reply?.kind !== "incremental-script-results")
          throw new Error(`${input.name}: ${JSON.stringify(reply)}`);
        return reply;
      };
      const cold = run(2_000_000, true);
      assert.equal(cold.results.length, expected.results.length);
      for (const [query, result] of cold.results.entries()) {
        const fresh: Solution = expected.results[query]!;
        if (result.stats.work === 0 && fresh.stats.work > 0)
          assert.deepEqual(
            answer(result),
            answer(fresh),
            `${input.name}-${query}`,
          );
        else assert.deepEqual(result, fresh, `${input.name}-${query}`);
      }
      cache = cold.cache;
      const warm = run(0, true);
      const complete = expected.results.filter(
        (result) => result.status !== "unknown",
      ).length;
      assert.equal(warm.stats.hits, complete, input.name);
      for (const [query, result] of warm.results.entries()) {
        const fresh: Solution = expected.results[query]!;
        if (fresh.status === "unknown") assert.equal(result.status, "unknown");
        else {
          assert.deepEqual(answer(result), answer(fresh));
          assert.equal(result.stats.work, 0);
        }
      }
      const disabled = run(2_000_000, false);
      assert.deepEqual(disabled.results, expected.results, input.name);
      assert.deepEqual(disabled.cache, cache, input.name);
      transcript.push(cold, warm, disabled);
      if (name === Object.keys(engines)[0]) hits += warm.stats.hits;
    }
    transcripts[name] = transcript;
    onRecord({ scriptCache: name, transcript });
  }
  const [first, ...rest] = Object.values(transcripts);
  for (const transcript of rest) assert.deepEqual(transcript, first);
  return { cacheScripts: inputs.length, cacheWarmHits: hits };
};

export const checks = async (): Promise<void> => {
  const bytes = await readFile(wasmPath);
  const wasm = await createWasm(bytes);
  const engines: Record<string, Engine> = {
    native: nativeEngine(nativePath),
    wasm: (requests) => requests.map((request) => wasm.request(request)),
  };
  const buildDigest = createHash("sha256").update(bytes).digest("hex");
  verifyScriptCache(
    [...handCases(), ...generated(0x74c1, 4)],
    engines,
    buildDigest,
    () => {},
  );
  const [tampered] = engines.wasm!([
    {
      kind: "run-script-incremental",
      source: handCases()[0]!.source,
      maxWork: 0,
      enabled: true,
      buildDigest,
      maxEntries: 4096,
      cache: { format: "rusmt.check-results", version: 2 },
    },
  ]);
  assert.equal(tampered?.kind, "error");
};
