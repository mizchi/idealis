import { isDeepStrictEqual } from "node:util";
import type { ReplayRecording } from "./replay.js";

type ItfValue = string | boolean | ItfRecord | ItfValue[];
interface ItfRecord {
  readonly [key: string]: ItfValue;
}

const variables = ["config", "command", "observation"] as const;
const initCommand = { kind: "init" } as const;

interface ReplayItf {
  readonly "#meta": {
    readonly format: "ITF";
    readonly source: string;
    readonly rmodelReplay: {
      readonly version: 1;
      readonly initial: ItfRecord;
      readonly commands: readonly ItfRecord[];
      readonly derive?: readonly string[];
    };
  };
  readonly vars: typeof variables;
  readonly states: readonly {
    readonly "#meta": { readonly index: number };
    readonly config: ItfRecord;
    readonly command: ItfRecord;
    readonly observation: ItfRecord;
  }[];
}

function record(value: unknown): Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    ![Object.prototype, null].includes(Object.getPrototypeOf(value))
  ) {
    throw new Error("expected a JSON record in the ITF replay");
  }
  return value as Record<string, unknown>;
}

function mapFields<T>(
  value: Record<string, unknown>,
  convert: (field: unknown) => T,
): Record<string, T> {
  return Object.fromEntries(
    Object.entries(value).map(([name, field]) => {
      if (name.startsWith("#"))
        throw new Error(`unsupported ITF field ${name}`);
      return [name, convert(field)];
    }),
  );
}

function encode(value: unknown): ItfValue {
  if (typeof value === "boolean" || typeof value === "string") return value;
  if (value === null) return { "#tup": [] };
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || Object.is(value, -0))
      throw new Error("ITF replay expects a safe integer");
    return { "#bigint": String(value) };
  }
  if (Array.isArray(value)) return Array.from(value, encode);
  return mapFields(record(value), encode);
}

function decode(value: unknown): unknown {
  if (typeof value === "boolean" || typeof value === "string") return value;
  if (Array.isArray(value)) return value.map(decode);
  const entries = record(value);
  if (Object.keys(entries).length === 1 && "#bigint" in entries) {
    const decimal = entries["#bigint"];
    if (
      typeof decimal !== "string" ||
      !/^(?:0|-?[1-9]\d*)$/.test(decimal) ||
      !Number.isSafeInteger(Number(decimal))
    ) {
      throw new Error("invalid ITF replay integer");
    }
    return Number(decimal);
  }
  if (
    Object.keys(entries).length === 1 &&
    Array.isArray(entries["#tup"]) &&
    entries["#tup"].length === 0
  ) {
    return null;
  }
  return mapFields(entries, decode);
}

function checkLength(commands: readonly unknown[], states: readonly unknown[]) {
  if (!states.length || states.length > commands.length + 1)
    throw new Error("invalid ITF replay length");
}

/** Encode a finite state replay using the config/command/observation variables. */
export function toReplayItf(recording: ReplayRecording<unknown>): ReplayItf {
  const { config, commands, initial, model, derive } = recording.scenario;
  checkLength(commands, recording.observations);
  const encodedCommands = commands.map((command) =>
    mapFields(record(command), encode),
  );
  const encodedConfig = mapFields(record(config), encode);
  return {
    "#meta": {
      format: "ITF",
      source: model,
      rmodelReplay: {
        version: 1,
        initial: mapFields(record(initial), encode),
        commands: encodedCommands,
        ...(derive === undefined ? {} : { derive }),
      },
    },
    vars: variables,
    states: recording.observations.map((observation, index) => ({
      "#meta": { index },
      config: encodedConfig,
      command: index === 0 ? initCommand : encodedCommands[index - 1]!,
      observation: mapFields(record(observation), encode),
    })),
  };
}

/** Decode the finite replay profile and keep any unobserved command suffix. */
export function fromReplayItf(value: unknown): ReplayRecording<unknown> {
  const trace = record(value);
  const metadata = record(trace["#meta"]);
  const replay = record(metadata.rmodelReplay);
  const derive = replay.derive;
  if (
    metadata.format !== "ITF" ||
    typeof metadata.source !== "string" ||
    replay.version !== 1 ||
    "loop" in trace ||
    !isDeepStrictEqual(trace.vars, variables) ||
    !Array.isArray(trace.states) ||
    !Array.isArray(replay.commands) ||
    (derive !== undefined &&
      (!Array.isArray(derive) ||
        !derive.every((name) => typeof name === "string") ||
        new Set(derive).size !== derive.length))
  ) {
    throw new Error("expected a finite rmodel ITF replay");
  }
  checkLength(replay.commands, trace.states);
  const commands = replay.commands.map((command) => record(decode(command)));
  const config = record(decode(record(trace.states[0]).config));
  const observations = trace.states.map((frame, index) => {
    const state = record(frame);
    const expected = index === 0 ? initCommand : commands[index - 1];
    if (
      record(state["#meta"]).index !== index ||
      !isDeepStrictEqual(decode(state.config), config) ||
      !isDeepStrictEqual(decode(state.command), expected)
    ) {
      throw new Error(`inconsistent ITF replay frame ${index}`);
    }
    return record(decode(state.observation));
  });
  return {
    scenario: {
      model: metadata.source,
      config,
      initial: record(decode(replay.initial)),
      commands,
      ...(derive === undefined ? {} : { derive }),
    },
    observations,
  };
}
