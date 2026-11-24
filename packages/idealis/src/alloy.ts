import type {
  AlloyAnalysis,
  AlloyCommand,
  AlloyInput,
  AlloyInstance,
  AlloyInstanceCheck,
  AlloyRuntime,
  AlloyStats,
} from "./contracts.js";
import { isRecord } from "./trace.js";

export type {
  AlloyAnalysis,
  AlloyCommand,
  AlloyInput,
  AlloyInstance,
  AlloyInstanceCheck,
  AlloyRuntime,
  AlloyStats,
} from "./contracts.js";

export class AlloyError extends Error {
  override readonly name = "AlloyError";
  constructor(
    readonly kind:
      | "syntax"
      | "unsupported"
      | "type"
      | "scope"
      | "resource"
      | "protocol",
    message: string,
  ) {
    super(message);
  }
}

const fields = (
  input: unknown,
  names: readonly string[],
): Record<string, unknown> => {
  if (
    !isRecord(input) ||
    Object.keys(input).length !== names.length ||
    names.some((name) => !Object.hasOwn(input, name))
  )
    throw new AlloyError("protocol", "Unexpected Alloy fields");
  return input;
};
const count = (input: unknown, maximum = Number.MAX_SAFE_INTEGER): number => {
  if (
    typeof input !== "number" ||
    !Number.isSafeInteger(input) ||
    input < 0 ||
    input > maximum
  )
    throw new AlloyError("protocol", "Invalid Alloy count");
  return input;
};
const text = (input: unknown): string => {
  if (typeof input !== "string")
    throw new AlloyError("protocol", "Expected Alloy text");
  return input;
};
const instance = (input: unknown): AlloyInstance => {
  const value = fields(input, ["signatures", "fields"]);
  if (!isRecord(value.signatures) || !isRecord(value.fields))
    throw new AlloyError("protocol", "Invalid Alloy relations");
  return Object.freeze({
    signatures: Object.freeze(
      Object.fromEntries(
        Object.entries(value.signatures).map(([name, atoms]) => {
          if (!Array.isArray(atoms))
            throw new AlloyError("protocol", "Invalid Alloy atoms");
          return [name, Object.freeze(atoms.map(text))];
        }),
      ),
    ),
    fields: Object.freeze(
      Object.fromEntries(
        Object.entries(value.fields).map(([name, tuples]) => {
          if (!Array.isArray(tuples))
            throw new AlloyError("protocol", "Invalid Alloy tuples");
          return [
            name,
            Object.freeze(
              tuples.map((tuple: unknown): readonly string[] => {
                if (!Array.isArray(tuple) || tuple.length < 2)
                  throw new AlloyError(
                    "protocol",
                    "Expected an Alloy tuple of at least two atoms",
                  );
                return Object.freeze(tuple.map(text));
              }),
            ),
          ];
        }),
      ),
    ),
  });
};
const command = (input: unknown): AlloyCommand => {
  const value = fields(input, ["label", "kind", "scope"]);
  const scope = fields(value.scope, ["signatures", "integerBitwidth"]);
  if (
    (value.kind !== "run" && value.kind !== "check") ||
    !Array.isArray(scope.signatures)
  )
    throw new AlloyError("protocol", "Invalid Alloy command");
  const signatures = scope.signatures.map((bound: unknown) => {
    const value = fields(bound, ["name", "maximum", "exact"]);
    if (typeof value.exact !== "boolean")
      throw new AlloyError("protocol", "Invalid Alloy exact scope");
    return Object.freeze({
      name: text(value.name),
      maximum: count(value.maximum, 8),
      exact: value.exact,
    });
  });
  if (
    signatures.length > 8 ||
    new Set(signatures.map((signature) => signature.name)).size !==
      signatures.length
  )
    throw new AlloyError("protocol", "Invalid Alloy signature scopes");
  return Object.freeze({
    label: text(value.label),
    kind: value.kind,
    scope: Object.freeze({
      signatures: Object.freeze(signatures),
      integerBitwidth: count(scope.integerBitwidth, 5),
    }),
  });
};
const stats = (input: unknown): AlloyStats => {
  const value = fields(input, ["work", "candidates", "factInstances"]);
  const result = {
    work: count(value.work),
    candidates: count(value.candidates),
    factInstances: count(value.factInstances),
  };
  if (result.factInstances > result.candidates)
    throw new AlloyError("protocol", "Invalid Alloy statistics");
  return Object.freeze(result);
};
const analysis = (input: unknown): AlloyAnalysis => {
  const value = fields(input, ["command", "status", "instance", "stats"]);
  if (
    value.status !== "instance" &&
    value.status !== "counterexample" &&
    value.status !== "no-instance" &&
    value.status !== "no-counterexample" &&
    value.status !== "unknown"
  )
    throw new AlloyError("protocol", "Invalid Alloy analysis status");
  const metadata = command(value.command);
  if (
    (value.status === "instance" && metadata.kind !== "run") ||
    (value.status === "counterexample" && metadata.kind !== "check") ||
    (value.status === "no-counterexample" && metadata.kind !== "check")
  )
    throw new AlloyError("protocol", "Alloy status and command differ");
  const witness =
    value.status === "instance" || value.status === "counterexample";
  if (!witness && value.instance !== null)
    throw new AlloyError("protocol", "Unexpected Alloy instance");
  return Object.freeze({
    command: metadata,
    status: value.status,
    instance: witness ? instance(value.instance) : null,
    stats: stats(value.stats),
  });
};
const checked = (input: unknown): AlloyInstanceCheck => {
  const value = fields(input, ["command", "status", "reason", "stats"]);
  if (
    value.status !== "matched" &&
    value.status !== "mismatch" &&
    value.status !== "invalid" &&
    value.status !== "unknown"
  )
    throw new AlloyError("protocol", "Invalid Alloy check status");
  if (
    value.reason !== null &&
    value.reason !== "scope" &&
    value.reason !== "declaration" &&
    value.reason !== "facts" &&
    value.reason !== "property" &&
    value.reason !== "work"
  )
    throw new AlloyError("protocol", "Invalid Alloy check reason");
  if (
    (value.status === "matched" && value.reason !== null) ||
    (value.status === "mismatch" && value.reason !== "property") ||
    (value.status === "unknown" && value.reason !== "work") ||
    (value.status === "invalid" &&
      value.reason !== "scope" &&
      value.reason !== "declaration" &&
      value.reason !== "facts")
  )
    throw new AlloyError("protocol", "Alloy status and reason differ");
  return Object.freeze({
    command: command(value.command),
    status: value.status,
    reason: value.reason,
    stats: stats(value.stats),
  });
};
const query = (input: AlloyInput) => {
  const value = fields(input, ["source", "command", "maxWork"]);
  return {
    source: text(value.source),
    command: count(value.command, 0xffff_ffff),
    maxWork: count(value.maxWork),
  };
};

export const createAlloyWasm = async (
  input: Uint8Array | WebAssembly.Module,
): Promise<AlloyRuntime> => {
  const module =
    input instanceof WebAssembly.Module
      ? input
      : await WebAssembly.compile(new Uint8Array(input).buffer);
  if (WebAssembly.Module.imports(module).length)
    throw new AlloyError(
      "protocol",
      "Alloy Wasm must not import host functions",
    );
  const wasm = await WebAssembly.instantiate(module);
  const memory = wasm.exports.memory;
  if (!(memory instanceof WebAssembly.Memory))
    throw new AlloyError("protocol", "Missing Alloy Wasm memory");
  const method = (name: string) => {
    const value = wasm.exports[`ralloy_${name}`];
    if (typeof value !== "function")
      throw new AlloyError("protocol", `Missing Alloy Wasm ${name}`);
    return (...args: number[]): number => {
      const result: unknown = value(...args);
      if (typeof result !== "number" || !Number.isInteger(result))
        throw new AlloyError("protocol", "Invalid Alloy Wasm integer");
      return result >>> 0;
    };
  };
  if (method("abi_version")() !== 1)
    throw new AlloyError("protocol", "Unsupported Alloy Wasm ABI");
  const resize = method("input_resize");
  const dispatch = wasm.exports.ralloy_dispatch;
  if (typeof dispatch !== "function")
    throw new AlloyError("protocol", "Missing Alloy Wasm dispatch");
  const outputPointer = method("output_ptr");
  const outputLength = method("output_len");
  const request = (request: object, kind: string): unknown => {
    const bytes = new TextEncoder().encode(
      JSON.stringify({ version: 1, request }),
    );
    const pointer = resize(bytes.length);
    if (pointer + bytes.length > memory.buffer.byteLength)
      throw new AlloyError("protocol", "Invalid Alloy input buffer");
    new Uint8Array(memory.buffer, pointer, bytes.length).set(bytes);
    dispatch();
    const start = outputPointer();
    const length = outputLength();
    if (start + length > memory.buffer.byteLength)
      throw new AlloyError("protocol", "Invalid Alloy output buffer");
    const reply: unknown = JSON.parse(
      new TextDecoder().decode(new Uint8Array(memory.buffer, start, length)),
    );
    if (isRecord(reply) && reply.kind === "error") {
      const error = fields(fields(reply, ["kind", "error"]).error, [
        "kind",
        "message",
      ]);
      if (
        error.kind !== "syntax" &&
        error.kind !== "unsupported" &&
        error.kind !== "type" &&
        error.kind !== "scope" &&
        error.kind !== "resource" &&
        error.kind !== "protocol"
      )
        throw new AlloyError("protocol", "Unknown Alloy error kind");
      throw new AlloyError(error.kind, text(error.message));
    }
    const value = fields(reply, ["kind", "result"]);
    if (value.kind !== kind)
      throw new AlloyError("protocol", "Unexpected Alloy response");
    return value.result;
  };
  return Object.freeze({
    analyze: (input: AlloyInput) =>
      analysis(request({ kind: "analyze", ...query(input) }, "analysis")),
    checkInstance: (input: AlloyInput, observed: AlloyInstance) =>
      checked(
        request(
          {
            kind: "check-instance",
            ...query(input),
            instance: instance(observed),
          },
          "instance-check",
        ),
      ),
  });
};
