import assert from "node:assert/strict";
import { isDeepStrictEqual } from "node:util";
import { digest } from "./provenance.js";

// Record format fixed in evals/plans/0074c-smt-compact-cache-records.md.
type Json = Record<string, unknown>;
type Snapshot = { readonly entries: Json } & Json;
type Delta = {
  readonly digest: string;
  readonly entries: number;
  readonly header?: Json;
  readonly added: Json;
  readonly removed: readonly string[];
};

const isObject = (value: unknown): value is Json =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const canonical = (value: unknown): unknown =>
  Array.isArray(value)
    ? value.map(canonical)
    : isObject(value)
      ? Object.fromEntries(
          Object.keys(value)
            .sort()
            .map((key) => [key, canonical(value[key])]),
        )
      : value;
const snapshotDigest = (snapshot: Snapshot) =>
  digest(JSON.stringify(canonical(snapshot)));
const split = ({ entries, ...header }: Snapshot) => ({ entries, header });

/** Each engine's script cache and the context cache are separate streams. */
const stream = (record: unknown) =>
  isObject(record)
    ? typeof record.scriptCache === "string"
      ? { key: `script:${record.scriptCache}`, field: "transcript" }
      : typeof record.contextCache === "string"
        ? { key: "context", field: "replies" }
        : undefined
    : undefined;

const mapCaches = (
  records: readonly unknown[],
  convert: (key: string, cache: unknown) => unknown,
) =>
  records.map((record) => {
    const selected = stream(record);
    if (!selected) return record;
    const replies = (record as Json)[selected.field];
    assert.ok(Array.isArray(replies), selected.key);
    return {
      ...(record as Json),
      [selected.field]: replies.map((reply) =>
        isObject(reply) && "cache" in reply
          ? { ...reply, cache: convert(selected.key, reply.cache) }
          : reply,
      ),
    };
  });

/** Replaces each cache snapshot by its change from the stream's previous one. */
export const compactRecords = (records: readonly unknown[]): unknown[] => {
  const previous = new Map<string, Snapshot>();
  return mapCaches(records, (key, cache) => {
    assert.ok(isObject(cache) && isObject(cache.entries), key);
    const snapshot = cache as Snapshot;
    const { entries, header } = split(snapshot);
    const before = previous.get(key);
    previous.set(key, snapshot);
    const old = before?.entries ?? {};
    const delta: Delta = {
      digest: snapshotDigest(snapshot),
      entries: Object.keys(entries).length,
      ...(before && isDeepStrictEqual(split(before).header, header)
        ? {}
        : { header }),
      added: Object.fromEntries(
        Object.entries(entries).filter(
          ([name, value]) => !isDeepStrictEqual(old[name], value),
        ),
      ),
      removed: Object.keys(old).filter((name) => !(name in entries)),
    };
    return delta;
  });
};

/** Rebuilds every snapshot and checks it against the recorded digest. */
export const expandRecords = (records: readonly unknown[]): unknown[] => {
  const previous = new Map<string, Snapshot>();
  return mapCaches(records, (key, value) => {
    const delta = value as Delta;
    const before = previous.get(key);
    const header = delta.header ?? (before && split(before).header);
    assert.ok(header, `${key}: snapshot without a header`);
    const entries: Json = { ...before?.entries, ...delta.added };
    for (const name of delta.removed) delete entries[name];
    const snapshot: Snapshot = {
      ...header,
      entries: Object.fromEntries(
        Object.keys(entries)
          .sort()
          .map((name) => [name, entries[name]]),
      ),
    };
    assert.equal(snapshotDigest(snapshot), delta.digest, `${key}: digest`);
    assert.equal(Object.keys(entries).length, delta.entries, `${key}: entries`);
    previous.set(key, snapshot);
    return snapshot;
  });
};
