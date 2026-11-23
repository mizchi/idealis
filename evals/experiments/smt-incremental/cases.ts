/** Cases of each bench binary; solver builds against both library versions. */
export const benches = {
  solver: [
    "parse-solve-4",
    "prepared-4",
    "parse-solve-32",
    "prepared-32",
    "parse-solve-128",
    "prepared-128",
    "parse-solve-ite",
    "context-assert-32",
    "context-constant-32",
    "context-assert-256",
    "context-constant-256",
    "script-parse-repeat-64",
    "script-prepared-repeat-64",
    "script-parse-scopes-32",
    "script-prepared-scopes-32",
    "script-parse-ite-64",
    "script-prepared-ite-64",
    "context-scopes-32",
  ],
  cache: [
    "prepared-4",
    "cache-miss-4",
    "prepared-32",
    "cache-miss-32",
    "prepared-128",
    "cache-miss-128",
    "script-prepared-repeat-64",
    "script-cache-cold-repeat-64",
    "script-cache-warm-repeat-64",
    "script-prepared-scopes-32",
    "script-cache-cold-scopes-32",
    "script-cache-warm-scopes-32",
    "script-prepared-ite-64",
    "script-cache-cold-ite-64",
    "script-cache-warm-ite-64",
  ],
  // Plan 0075: the cache snapshot passed through the JSON protocol.
  protocol: [
    "protocol-check",
    ...[0, 64, 512, 4096].flatMap((count) => [
      `protocol-miss-${count}`,
      `protocol-hit-${count}`,
      `store-hit-${count}`,
      `store-miss-${count}`,
      `handle-hit-${count}`,
      `handle-miss-${count}`,
    ]),
  ],
} as const;
export type Bench = keyof typeof benches;
export const isBench = (value: unknown): value is Bench =>
  value === "solver" || value === "cache" || value === "protocol";
