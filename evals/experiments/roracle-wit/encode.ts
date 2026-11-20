import type { OracleRequest } from "../../../packages/idealis/src/generated/roracle.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import type {
  Batch,
  Observation,
} from "./generated/interfaces/idealis-oracle-protocol.js";

const encoder = new TextEncoder();
export class Writer {
  data = new Uint8Array(1024);
  length = 0;
  reserve(size: number): void {
    if (size > 64 * 1024 * 1024 - this.length)
      throw new Error("Typed buffer exceeds 64 MiB");
    if (this.length + size <= this.data.length) return;
    const data = new Uint8Array(
      Math.max(this.length + size, this.data.length * 2),
    );
    data.set(this.data);
    this.data = data;
  }
  word(value: number): void {
    this.reserve(4);
    new DataView(this.data.buffer).setUint32(this.length, value, true);
    this.length += 4;
  }
  byte(value: number): void {
    this.reserve(1);
    this.data[this.length++] = value;
  }
  raw(value: Uint8Array): void {
    this.reserve(value.length);
    this.data.set(value, this.length);
    this.length += value.length;
  }
  text(value: string): void {
    const bytes = encoder.encode(value);
    this.word(bytes.length);
    this.raw(bytes);
  }
  finish(): Uint8Array {
    return this.data.slice(0, this.length);
  }
}

export const encodeBatch = (input: OracleRequest): Batch => {
  const writer = new Writer();
  const ancestors = new Set<object>();
  let remaining = 1_000_000;
  const budget = (count: number) => {
    remaining -= count;
    if (remaining < 0) throw new Error("Typed node budget exceeded");
  };
  const unicode = (value: string) => {
    if (/[\uD800-\uDFFF]/u.test(value))
      throw new Error("Invalid Unicode scalar string");
    return value;
  };
  const text = (value: unknown) => {
    budget(1);
    if (typeof value !== "string") throw new Error("Expected text");
    return unicode(value);
  };
  const array = (value: unknown, counted = false): readonly unknown[] => {
    if (!counted) budget(1);
    if (
      !Array.isArray(value) ||
      Reflect.ownKeys(value).length !== value.length + 1 ||
      Object.getOwnPropertySymbols(value).length
    )
      throw new Error("Invalid dense array");
    for (let index = 0; index < value.length; index++) {
      const property = Object.getOwnPropertyDescriptor(value, index);
      if (!property || !("value" in property))
        throw new Error("Array accessor or hole");
    }
    return value;
  };
  const record = (
    value: unknown,
    names: readonly string[],
    counted = false,
  ): Record<string, unknown> => {
    budget((counted ? 0 : 1) + names.length);
    if (
      !isRecord(value) ||
      Object.getPrototypeOf(value) !== Object.prototype ||
      Reflect.ownKeys(value).length !== names.length
    )
      throw new Error("Invalid record fields");
    for (const name in value)
      if (!Object.hasOwn(value, name))
        throw new Error("Inherited enumerable field");
    for (const name of names) {
      unicode(name);
      const property = Object.getOwnPropertyDescriptor(value, name);
      if (!property?.enumerable || !("value" in property))
        throw new Error("Record accessor or hidden field");
    }
    return value;
  };
  const value = (input: unknown, depth: number): number => {
    budget(1);
    if (depth > 128) throw new Error("Typed depth budget exceeded");
    const start = writer.length;
    writer.word(0);
    if (input === null) writer.byte(0);
    else if (typeof input === "boolean") writer.byte(input ? 2 : 1);
    else if (typeof input === "number") {
      if (!Number.isFinite(input) || Object.is(input, -0))
        throw new Error("Noncanonical number; use snapshot");
      writer.byte(3);
      writer.raw(encoder.encode(JSON.stringify(input)));
    } else if (typeof input === "string") {
      writer.byte(4);
      writer.raw(encoder.encode(unicode(input)));
    } else if (typeof input === "object") {
      if (ancestors.has(input)) throw new Error("Cyclic typed value");
      ancestors.add(input);
      if (Array.isArray(input)) {
        const values = array(input, true);
        writer.byte(5);
        writer.word(values.length);
        for (const child of values) value(child, depth + 1);
      } else {
        const names = Reflect.ownKeys(input)
          .map((name) => {
            if (typeof name !== "string") throw new Error("Symbol key");
            return name;
          })
          .sort();
        const fields = record(input, names, true);
        writer.byte(6);
        writer.word(names.length);
        for (const name of names) {
          writer.text(name);
          value(fields[name], depth + 1);
        }
      }
      ancestors.delete(input);
    } else throw new Error("Non-JSON typed value");
    new DataView(writer.data.buffer).setUint32(
      start,
      writer.length - start,
      true,
    );
    return start;
  };
  const observation = (input: unknown): Observation => {
    if (!input || typeof input !== "object" || !Object.hasOwn(input, "kind"))
      throw new Error("Invalid observation");
    const property = Object.getOwnPropertyDescriptor(input, "kind");
    if (!property || !("value" in property))
      throw new Error("Observation accessor");
    const kind: unknown = property.value;
    const row = record(input, [
      "kind",
      kind === "value"
        ? "value"
        : kind === "allowed"
          ? "values"
          : kind === "error"
            ? "message"
            : "reason",
    ]);
    text(row.kind);
    switch (kind) {
      case "value":
        return { tag: "value", val: value(row.value, 0) };
      case "allowed":
        return {
          tag: "allowed",
          val: new Uint32Array(
            array(row.values).map((input) => value(input, 0)),
          ),
        };
      case "unknown":
        return { tag: "unknown", val: text(row.reason) };
      case "invalid":
        return { tag: "invalid", val: text(row.reason) };
      case "error":
        return { tag: "error", val: text(row.message) };
      default:
        throw new Error("Invalid observation kind");
    }
  };
  const request = record(input, [
    "version",
    "group",
    "reference",
    "implementations",
    "cases",
  ]);
  budget(1);
  if (
    typeof request.version !== "number" ||
    !Number.isInteger(request.version) ||
    request.version < 0 ||
    request.version > 0xffffffff
  )
    throw new Error("Invalid u32 version");
  const group = text(request.group);
  const reference = text(request.reference);
  const implementations = array(request.implementations).map(text);
  const cases = array(request.cases).map((input) => {
    const row = record(input, ["id", "input", "reference", "actual"]);
    return {
      id: text(row.id),
      input: value(row.input, 0),
      reference: observation(row.reference),
      actual: array(row.actual).map(observation),
    };
  });
  return {
    version: request.version,
    group,
    reference,
    implementations,
    cases,
    payload: writer.finish(),
  };
};

export const encodeBuffer = (batch: Batch): Uint8Array => {
  const writer = new Writer();
  writer.word(0x42524f49);
  writer.word(1);
  writer.word(batch.version);
  writer.text(batch.group);
  writer.text(batch.reference);
  writer.word(batch.implementations.length);
  for (const id of batch.implementations) writer.text(id);
  writer.word(batch.cases.length);
  const observation = (input: Observation) => {
    switch (input.tag) {
      case "value":
        writer.word(0);
        writer.word(input.val);
        break;
      case "allowed":
        writer.word(1);
        writer.word(input.val.length);
        for (const root of input.val) writer.word(root);
        break;
      case "unknown":
        writer.word(2);
        writer.text(input.val);
        break;
      case "invalid":
        writer.word(3);
        writer.text(input.val);
        break;
      case "error":
        writer.word(4);
        writer.text(input.val);
        break;
    }
  };
  for (const input of batch.cases) {
    writer.text(input.id);
    writer.word(input.input);
    observation(input.reference);
    writer.word(input.actual.length);
    for (const actual of input.actual) observation(actual);
  }
  writer.word(batch.payload.length);
  writer.raw(batch.payload);
  return writer.finish();
};
