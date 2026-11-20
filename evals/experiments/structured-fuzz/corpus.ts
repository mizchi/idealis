import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { gunzipSync } from "node:zlib";
import type { Input } from "./clients.js";

const savedCase = {
  format: "idealis.test-case",
  version: 1,
  rng: "xorshift32-v1",
  generatorId: "g",
  generatorVersion: "1",
  seed: 1,
  index: 0,
  actions: ["bad"],
};
export const regression: readonly Input[] = [
  { mode: "structured", bytes: [1, 0, 1, 4, 8, 13, 15] },
  { mode: "structured", bytes: [0, 10, 15, 8, 12, 1, 11, 2, 3] },
  { mode: "structured", bytes: [1, 9, 4, 8, 0, 0, 4, 0, 15] },
  {
    mode: "raw",
    bytes: Array.from(
      new TextEncoder().encode(
        [
          JSON.stringify({
            version: 1,
            request: {
              kind: "replay",
              case: savedCase,
              expectedSignature: null,
            },
          }),
          JSON.stringify({
            version: 1,
            request: {
              kind: "resume",
              id: 1,
              verdict: { status: "matched", extra: true },
            },
          }),
          "invalid-json",
        ].join("\n"),
      ),
    ),
  },
];
export const sources = [
  "evals/runs/0036-testing-development/exchanges.json.gz",
  "evals/runs/0038-schedules-development/records.json.gz",
];
export const prepare = async (): Promise<
  Readonly<Record<"raw" | "structured", readonly Uint8Array[]>>
> => {
  const exchanges: { request: { kind: string } }[] = JSON.parse(
    gunzipSync(await readFile(sources[0] ?? "")).toString(),
  );
  const raw: Uint8Array[] = regression
    .filter((input) => input.mode === "raw")
    .map((input) => Uint8Array.from(input.bytes));
  for (const [index, exchange] of exchanges.entries()) {
    if (
      !["property", "replay", "shrink"].includes(exchange.request.kind) ||
      raw.length >= 9
    )
      continue;
    const lines: string[] = [];
    for (const next of exchanges.slice(index, index + 16)) {
      const line = JSON.stringify({ version: 1, request: next.request });
      if (new TextEncoder().encode([...lines, line].join("\n")).length > 8192)
        break;
      lines.push(line);
    }
    if (lines.length) raw.push(new TextEncoder().encode(lines.join("\n")));
  }
  const structured: Uint8Array[] = regression
    .filter((input) => input.mode === "structured")
    .map((input) => Uint8Array.from(input.bytes));
  const records: {
    kind: string;
    broken?: boolean;
    result?: { counterexample?: unknown };
  }[] = JSON.parse(gunzipSync(await readFile(sources[1] ?? "")).toString());
  for (const record of records
    .filter((record) => record.kind === "property" && record.broken)
    .slice(0, 4)) {
    const bytes = new TextEncoder().encode(
      JSON.stringify(record.result?.counterexample),
    );
    assert.ok(bytes.length > 0 && bytes.length <= 4096);
    structured.push(
      Uint8Array.from([
        1,
        16,
        bytes.length & 255,
        bytes.length >>> 8,
        ...bytes,
        15,
        0,
        3,
        8,
        1,
        15,
      ]),
    );
  }
  return { raw, structured };
};
export const probes = (seed: number, count: number): readonly Uint8Array[] => {
  let state = seed >>> 0;
  const next = () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return state >>> 0;
  };
  return Array.from({ length: count }, () =>
    Uint8Array.from(
      Array.from({ length: 1 + (next() % 1024) }, () => next() & 255),
    ),
  );
};
