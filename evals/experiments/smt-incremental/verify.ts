import { readFile } from "node:fs/promises";
import { nativeEngine, verifyScriptCache } from "../smt-cache/checks.js";
import type { ScriptInput } from "../smt-scripts/corpus.js";
import { createZ3, processClient } from "../smt-subset/clients.js";
import { nativePath, wasmPath } from "../smt-subset/paths.js";
import { digest } from "../smt-subset/provenance.js";
import { createWasm } from "../smt-subset/wasm.js";
import { decode } from "../smt-subset/wire.js";
import { verify as verifyTuning } from "../smt-tuning/verify.js";
import {
  type Client,
  type ContextSequence,
  verifyContexts,
} from "./context.js";

export interface Inputs {
  readonly scripts: readonly ScriptInput[];
  readonly contexts: readonly ContextSequence[];
}
const persistent = (path: string) => {
  const process = processClient(path, [], true);
  const client: Client = async (request) =>
    decode(
      JSON.parse(
        await process.request(JSON.stringify({ version: 1, request })),
      ),
    );
  return { client, close: process.close };
};

/**
 * 0042's script/fixture/Z3 checks against the baseline, generated context
 * sequences against the baseline and Z3, and the check cache on both.
 * Without a baseline only the candidate engines are compared.
 */
export const verify = async (
  inputs: Inputs,
  onRecord: (record: unknown) => void,
  baseline?: { readonly native: string; readonly wasm: string },
) => {
  const bytes = await readFile(wasmPath);
  const candidate = await createWasm(bytes);
  const previous = baseline
    ? await createWasm(await readFile(baseline.wasm))
    : undefined;
  const scripts = await verifyTuning(
    inputs.scripts,
    candidate,
    onRecord,
    baseline?.native,
    previous,
  );
  const native = persistent(nativePath);
  const old = baseline ? persistent(baseline.native) : undefined;
  const z3 = await createZ3();
  const wasmClient: Client = async (request) => candidate.request(request);
  try {
    const buildDigest = digest(bytes);
    const contexts = await verifyContexts(
      inputs.contexts,
      {
        native: native.client,
        wasm: wasmClient,
        ...(old && previous
          ? {
              baselineNative: old.client,
              baselineWasm: async (request) => previous.request(request),
            }
          : {}),
      },
      { native: native.client, wasm: wasmClient },
      z3,
      buildDigest,
      onRecord,
    );
    const cache = verifyScriptCache(
      inputs.scripts,
      {
        native: nativeEngine(nativePath),
        wasm: (requests) =>
          requests.map((request) => candidate.request(request)),
      },
      buildDigest,
      onRecord,
    );
    return { ...scripts, ...contexts, ...cache, z3Version: z3.version };
  } finally {
    await z3.close();
    await Promise.all([native.close(), old?.close()]);
  }
};
