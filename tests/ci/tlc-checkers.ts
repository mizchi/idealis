import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { check } from "tla-ts";
import { startNative } from "../../evals/experiments/usable/native.js";
import type {
  PortableCommand,
  PortableCore,
  PortableOutput,
} from "../../packages/idealis/src/contracts.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import { nativePath, tlcBuild, wasmPath } from "./tlc-build.js";
import { createResultCache } from "./tlc-result-cache.js";

const root = process.env.IDEALIS_TLC_RESULTS;
const directory = root && join(root, tlcBuild());
const complete = (reply: unknown) =>
  isRecord(reply) &&
  reply.kind === "safety" &&
  isRecord(reply.result) &&
  reply.result.status === "complete" &&
  typeof reply.result.states === "number" &&
  reply.result.states > 0 &&
  reply.result.reason === null &&
  reply.result.violation === null;
const nativeCache = createResultCache<unknown>(
  directory,
  "rtlc-native",
  complete,
  true,
);
const wasmCache = createResultCache<PortableOutput>(
  directory,
  "rtlc-wasm",
  complete,
  true,
);
const referenceCache = createResultCache<Awaited<ReturnType<typeof check>>>(
  directory,
  "tla-ts",
  (reply) =>
    reply.status === "success" &&
    reply.distinctStates > 0 &&
    reply.queuedStates === 0 &&
    !reply.violation &&
    !reply.failure,
  true,
);
const cacheable = ({ request }: PortableCommand) =>
  request.kind === "check-safety" || request.kind === "check-tla";

export const createCiWasm = async (): Promise<PortableCore> => {
  const core = await createPortableWasm(await readFile(wasmPath));
  return {
    request: (command) =>
      cacheable(command)
        ? wasmCache.runSync(command, () => core.request(command))
        : core.request(command),
  };
};

export const startCiNative = () => {
  const native = startNative(nativePath);
  return {
    ...native,
    request: (command: PortableCommand) =>
      cacheable(command)
        ? nativeCache.run(command, () => native.request(command))
        : native.request(command),
  };
};

export const ciNativeResult = (command: PortableCommand, run: () => unknown) =>
  cacheable(command) ? nativeCache.runSync(command, run) : run();

// Only the self-contained, deterministic reference models in these regression suites opt in.
export const checkCiReference: typeof check = (options) => {
  if (
    !directory ||
    (options.workers ?? 1) !== 1 ||
    typeof options.config !== "string" ||
    Object.values(options).some((value) => typeof value === "function") ||
    /\b(?:TLCGet|TLCSet|RandomElement|RandomSubset|RandomSetOfSubsets|JavaTime)\b/u.test(
      options.source + "\n" + options.config,
    )
  )
    return check(options);
  return referenceCache.run(options, () => check(options));
};
