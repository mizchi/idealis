import assert from "node:assert/strict";
import type { CheckCache, Reply, Request } from "../smt-subset/wire.js";

// Cases fixed in evals/plans/0075-smt-cache-transport.md; they mirror
// crates/rusmt/benches/protocol.rs.
export const buildDigest = "0123456789abcdef".repeat(4);
export const counts = [0, 64, 512, 4096] as const;
const maxWork = 2_000_000;
const maxEntries = 8192;
export const chain = (count: number) => {
  let source = "";
  for (let index = 0; index < count; index++)
    source += `(declare-const x${index} Int)`;
  source += "(assert (= x0 0))";
  for (let index = 1; index < count; index++)
    source += `(assert (= x${index} (+ x${index - 1} 1)))`;
  return source;
};
type Client = { readonly request: (request: Request) => Reply };
export interface ProtocolCase {
  readonly name: string;
  readonly request: Request;
  /** Filler entries in the snapshot, excluding the measured query. */
  readonly entries: number;
  readonly cacheBytes: number;
}

const handleOf = (client: Client, source: string) => {
  const reply = client.request({ kind: "prepare", source });
  assert.equal(reply.kind, "prepared");
  return (reply as Extract<Reply, { kind: "prepared" }>).handle;
};
const checked = (client: Client, handle: number, cache?: CheckCache) => {
  const reply = client.request({
    kind: "check-incremental",
    handle,
    maxWork,
    enabled: true,
    buildDigest,
    maxEntries,
    ...(cache ? { cache } : {}),
  });
  assert.equal(reply.kind, "incremental-solution");
  return (reply as Extract<Reply, { kind: "incremental-solution" }>).cache;
};

/** Builds every request once; the timed loop only repeats them. */
export const wasmCases = (
  client: Client,
  sizes: readonly number[] = counts,
): ProtocolCase[] => {
  const handle = handleOf(client, chain(32));
  const cases: ProtocolCase[] = [
    {
      name: "wasm-check",
      request: { kind: "check", handle, maxWork },
      entries: 0,
      cacheBytes: 0,
    },
  ];
  let fillers: CheckCache | undefined;
  let filled = 0;
  for (const count of sizes) {
    for (; filled < count; filled++) {
      const other = handleOf(
        client,
        `(declare-const x Int)(assert (<= x ${filled}))`,
      );
      fillers = checked(client, other, fillers);
      assert.equal(
        client.request({ kind: "release", handle: other }).kind,
        "released",
      );
    }
    const hit = checked(client, handle, fillers);
    for (const [name, cache] of [
      ["miss", fillers],
      ["hit", hit],
    ] as const)
      cases.push({
        name: `wasm-${name}-${count}`,
        request: {
          kind: "check-incremental",
          handle,
          maxWork,
          enabled: true,
          buildDigest,
          maxEntries,
          cache: cache ?? {
            format: "rusmt.check-results",
            version: 1,
            buildDigest,
            entries: {},
          },
        },
        entries: count,
        cacheBytes: JSON.stringify(cache ?? {}).length,
      });
  }
  return cases;
};
