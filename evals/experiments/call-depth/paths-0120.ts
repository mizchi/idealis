import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import type {
  PortableCommand,
  SafetyProgram,
} from "../../../packages/idealis/src/contracts.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import {
  bounds,
  checked,
  compileFailure,
  type Input,
  type Outcome,
} from "../nesting-chains/outcome.js";
import { startNative } from "../usable/native.js";

export interface Probe {
  readonly peakDepth: number;
  readonly peakTotal: number;
  readonly hits256: number;
  readonly published256: unknown;
}

export interface PathCell {
  readonly outcome: Outcome;
  readonly code: string | null;
  readonly summary: {
    readonly status: string;
    readonly states: number;
    readonly generated: number;
    readonly work: number;
  } | null;
  readonly raw: string | null;
  readonly wire: string | null;
  readonly publishedWire: string | null;
  readonly programWire: string | null;
  readonly compileText: string | null;
  readonly trace: unknown;
  readonly probe: Probe | null;
  readonly ir: string | null;
  readonly irNormalized: string | null;
}

export type Reply = Record<string, unknown>;

const texts = new WeakMap<Reply, string>();
const programTexts = new WeakMap<object, string>();

const topLevelMembers = (
  text: string,
): { key: string; start: number; end: number }[] => {
  const members: { key: string; start: number; end: number }[] = [];
  let depth = 0;
  let index = 0;
  let memberStart = -1;
  let key = "";
  const skipString = (): string => {
    const begin = index;
    index++;
    while (text[index] !== '"') index += text[index] === "\\" ? 2 : 1;
    index++;
    return text.slice(begin, index);
  };
  while (index < text.length) {
    const char = text[index];
    if (char === '"') {
      const literal = skipString();
      if (depth === 1 && memberStart === -1 && text[index] === ":") {
        memberStart = text.lastIndexOf(literal, index);
        key = JSON.parse(literal) as string;
      }
      continue;
    }
    if (char === "{" || char === "[") depth++;
    else if (char === "}" || char === "]") {
      depth--;
      if (depth === 0 && memberStart !== -1) {
        members.push({ key, start: memberStart, end: index });
        memberStart = -1;
      }
    } else if (char === "," && depth === 1 && memberStart !== -1) {
      members.push({ key, start: memberStart, end: index });
      memberStart = -1;
    }
    index++;
  }
  return members;
};

export const splitProbeText = (
  text: string,
): { without: string; published: string | null } => {
  const members = topLevelMembers(text);
  const probe = members.find((member) => member.key === "probe");
  if (!probe) return { without: text, published: null };
  const body = text.slice(probe.start, probe.end);
  const value = body.slice(body.indexOf(":") + 1);
  const published = topLevelMembers(value).find(
    (member) => member.key === "published256",
  );
  const comma = text[probe.start - 1] === "," ? probe.start - 1 : probe.start;
  const after =
    comma === probe.start && text[probe.end] === ","
      ? probe.end + 1
      : probe.end;
  return {
    without: text.slice(0, comma) + text.slice(after),
    published: published
      ? value.slice(value.indexOf(":", published.start) + 1, published.end)
      : null,
  };
};
export type Request = (command: PortableCommand) => Promise<Reply>;

export const pathNames = [
  "ts-wasm",
  "ts-native",
  "rust-wasm",
  "rust-native",
] as const;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (isRecord(value))
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
      .join(",")}}`;
  return JSON.stringify(value) ?? "null";
};

const digest = (value: unknown): string =>
  createHash("sha256")
    .update(typeof value === "string" ? value : canonical(value))
    .digest("hex");

const normalize = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(normalize);
  if (!isRecord(value)) return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(
        ([key, entry]) =>
          !(key === "subscript" && entry === null) &&
          !(
            [
              "fairness",
              "initialProperties",
              "actionProperties",
              "properties",
            ].includes(key) &&
            Array.isArray(entry) &&
            entry.length === 0
          ),
      )
      .map(([key, entry]) => [key, normalize(entry)]),
  );
};

export const wasmRequest = async (path: string): Promise<Request> => {
  const bytes = await readFile(path);
  const module = await WebAssembly.compile(new Uint8Array(bytes));
  let instance = new WebAssembly.Instance(module, {});
  const send = (command: PortableCommand): Reply => {
    const exports = instance.exports as Record<string, WebAssembly.ExportValue>;
    const input = new TextEncoder().encode(JSON.stringify(command));
    const resize = exports.modelt_input_resize as (length: number) => number;
    const memory = exports.memory as WebAssembly.Memory;
    const pointer = resize(input.length) >>> 0;
    new Uint8Array(memory.buffer, pointer, input.length).set(input);
    try {
      (exports.modelt_dispatch as () => void)();
    } catch (error) {
      instance = new WebAssembly.Instance(module, {});
      throw error;
    }
    const output = new Uint8Array(
      memory.buffer,
      (exports.modelt_output_ptr as () => number)() >>> 0,
      (exports.modelt_output_len as () => number)() >>> 0,
    ).slice();
    const text = new TextDecoder().decode(output);
    const reply: unknown = JSON.parse(text);
    assert.ok(isRecord(reply));
    texts.set(reply, text);
    return reply;
  };
  return async (command) => send(command);
};

const probeOf = (reply: Reply): Probe | null => {
  const probe = reply.probe;
  if (!isRecord(probe)) return null;
  return {
    peakDepth: Number(probe.peakDepth),
    peakTotal: Number(probe.peakTotal),
    hits256: Number(probe.hits256),
    published256: probe.published256,
  };
};

export const stripProbe = (reply: Reply): Reply => {
  const { probe: _probe, ...rest } = reply;
  return rest;
};

export const traceOf = (reply: Reply): unknown => {
  const result = reply.result;
  if (!isRecord(result) || !isRecord(result.violation)) return null;
  return result.violation.trace ?? null;
};

const summaryOf = (reply: Reply): PathCell["summary"] => {
  const result = reply.result;
  if (!isRecord(result)) return null;
  return {
    status: String(result.status),
    states: Number(result.states),
    generated: Number(result.generated),
    work: Number(result.work),
  };
};

const observe = async (
  compile: (input: Input) => Promise<SafetyProgram>,
  check: (program: SafetyProgram) => Promise<Reply>,
  input: Input,
): Promise<PathCell> => {
  let program: SafetyProgram;
  try {
    program = await compile(input);
  } catch (error) {
    assert.ok(error instanceof Error);
    return {
      outcome: compileFailure(error.message),
      code: null,
      summary: null,
      raw: null,
      wire: null,
      publishedWire: null,
      programWire: null,
      compileText: error.message,
      trace: null,
      probe: null,
      ir: null,
      irNormalized: null,
    };
  }
  const reply = await check(program);
  const text = texts.get(reply);
  const split = text === undefined ? null : splitProbeText(text);
  return {
    outcome: checked(stripProbe(reply)),
    code: reply.kind === "error" ? String(reply.code) : null,
    summary: summaryOf(reply),
    raw: digest(stripProbe(reply)),
    wire: split === null ? null : digest(split.without),
    publishedWire:
      split?.published === null || split === null
        ? null
        : digest(split.published),
    programWire: digest(programTexts.get(program) ?? JSON.stringify(program)),
    compileText: null,
    trace: traceOf(reply),
    probe: probeOf(reply),
    ir: digest(program),
    irNormalized: digest(normalize(program)),
  };
};

export interface NamedCase {
  readonly name: string;
  readonly source: string;
  readonly config: string;
}

export const loadCases = async (
  files: readonly string[],
  quint: string,
): Promise<{ cases: NamedCase[]; duplicates: string[] }> => {
  const seen = new Map<string, NamedCase>();
  const duplicates: string[] = [];
  for (const file of files) {
    const parsed: unknown = JSON.parse(await readFile(file, "utf8"));
    assert.ok(Array.isArray(parsed));
    for (const entry of parsed as NamedCase[]) {
      const known = seen.get(entry.name);
      if (known) {
        if (known.source !== entry.source || known.config !== entry.config)
          duplicates.push(entry.name);
        continue;
      }
      seen.set(entry.name, {
        name: entry.name,
        source: entry.source,
        config: entry.config,
      });
    }
  }
  const cases = [...seen.values()];
  for (const module of ["LongConjunction", "LongIfChain"])
    for (const kind of ["safety", "broken"])
      cases.push({
        name: `quint:${module}-${kind}`,
        source: await readFile(join(quint, `${module}.tla`), "utf8"),
        config: await readFile(join(quint, `${module}-${kind}.cfg`), "utf8"),
      });
  return { cases, duplicates };
};

if (process.argv[1]?.endsWith("paths-0120.js")) {
  const [side, snapshot, toolDist, output, casesFiles, quint] =
    process.argv.slice(2);
  assert.ok(
    side && snapshot && toolDist && output && casesFiles && quint,
    "usage: paths-0120.js <side> <snapshot dir> <tool dist> <out.json> <cases.json,...> <quint dir>",
  );
  const typescript = (await import(
    pathToFileURL(join(toolDist, "packages/idealis/src/tla-safety.js")).href
  )) as {
    compileTlaSafety: (input: Input) => SafetyProgram;
  };
  const nativePath = join(snapshot, "idealis-core");
  let native = startNative(nativePath);
  const wasm = await wasmRequest(join(snapshot, "idealis_core.wasm"));
  const nativeRequest: Request = async (command) => {
    const text = await native.requestText(command);
    const reply: unknown = JSON.parse(text);
    assert.ok(isRecord(reply));
    texts.set(reply, text);
    return reply;
  };
  const compileWith =
    (request: Request) =>
    async (input: Input): Promise<SafetyProgram> => {
      const reply = await request({
        version: 1,
        request: { kind: "compile-tla", ...input },
      });
      if (reply.kind === "compiled") {
        const program = reply.program as SafetyProgram;
        const text = texts.get(reply);
        const member =
          text === undefined
            ? undefined
            : topLevelMembers(text).find((entry) => entry.key === "program");
        if (text !== undefined && member !== undefined)
          programTexts.set(
            program,
            text.slice(text.indexOf(":", member.start) + 1, member.end),
          );
        return program;
      }
      throw new Error(String(reply.message));
    };
  const checkWith =
    (request: Request) =>
    (program: SafetyProgram): Promise<Reply> =>
      request({
        version: 1,
        request: { kind: "check-safety", program, bounds },
      });
  const compileTs = async (input: Input): Promise<SafetyProgram> =>
    typescript.compileTlaSafety(input);
  const paths: Record<
    (typeof pathNames)[number],
    readonly [
      (input: Input) => Promise<SafetyProgram>,
      (program: SafetyProgram) => Promise<Reply>,
    ]
  > = {
    "ts-wasm": [compileTs, checkWith(wasm)],
    "ts-native": [compileTs, (program) => checkWith(nativeRequest)(program)],
    "rust-wasm": [compileWith(wasm), checkWith(wasm)],
    "rust-native": [
      compileWith(nativeRequest),
      (program) => checkWith(nativeRequest)(program),
    ],
  };
  const { cases, duplicates } = await loadCases(casesFiles.split(","), quint);
  const result: {
    side: string;
    count: number;
    duplicateSourceMismatches: string[];
    cases: Record<string, Record<string, Record<string, PathCell>>>;
    selfCheck: unknown;
  } = {
    side,
    count: cases.length,
    duplicateSourceMismatches: duplicates,
    cases: {},
    selfCheck: null,
  };
  const run = async (
    entry: NamedCase,
    irVersion: 1 | 2,
    name: (typeof pathNames)[number],
  ): Promise<PathCell> => {
    const [compile, check] = paths[name];
    try {
      return await observe(compile, check, {
        source: entry.source,
        config: entry.config,
        irVersion,
      });
    } catch (error) {
      const message = String(
        error instanceof Error ? error.message : error,
      ).slice(0, 300);
      if (message.includes("Native process")) native = startNative(nativePath);
      return {
        outcome: { stage: "check", message: `exception: ${message}` },
        code: null,
        summary: null,
        raw: null,
        wire: null,
        publishedWire: null,
        programWire: null,
        compileText: null,
        trace: null,
        probe: null,
        ir: null,
        irNormalized: null,
      };
    }
  };
  for (const entry of cases) {
    const row: Record<string, Record<string, PathCell>> = {};
    for (const irVersion of [1, 2] as const) {
      const cell: Record<string, PathCell> = {};
      for (const name of pathNames)
        cell[name] = await run(entry, irVersion, name);
      row[`v${irVersion}`] = cell;
    }
    result.cases[entry.name] = row;
  }
  const deep = cases.find((entry) => entry.name === "hop-invariant-168");
  const shallow = cases.find((entry) => entry.name === "hop-invariant-40");
  if (deep && shallow) {
    const recorded =
      result.cases[shallow.name]?.v2?.["rust-native"]?.probe ?? null;
    const afterDeep = await run(deep, 2, "rust-native");
    const afterShallow = await run(shallow, 2, "rust-native");
    result.selfCheck = {
      deepPeak: afterDeep.probe?.peakDepth ?? null,
      shallowPeakAfterDeep: afterShallow.probe?.peakDepth ?? null,
      shallowPeakRecorded: recorded?.peakDepth ?? null,
      carriesOver: afterShallow.probe?.peakDepth === afterDeep.probe?.peakDepth,
    };
  }
  try {
    await native.close();
  } catch (error) {
    assert.ok(error instanceof Error);
    (result as Record<string, unknown>).nativeCloseError = error.message;
  }
  await writeFile(output, `${JSON.stringify(result, null, 1)}\n`, {
    flag: "wx",
  });
  console.log(`wrote ${output}: ${cases.length} cases`);
}
