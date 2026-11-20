import assert from "node:assert/strict";

export const routes = ["tlc-1x8", "tlc-8x1", "native-8", "wasm-8"] as const;
export type Route = (typeof routes)[number];
export type TlcRoute = "tlc-1x8" | "tlc-8x1";
export const tlcSettings = (route: TlcRoute) =>
  route === "tlc-1x8"
    ? { jvmCount: 1, workers: 8, heapGiB: 8 }
    : { jvmCount: 8, workers: 1, heapGiB: 1 };
export const normalizeTlc = (stdout: string, exit: number | null) => {
  assert.equal(exit, 0, "TLC process failed");
  assert.match(stdout, /Model checking completed\. No error has been found\./);
  const counts = /(\d+) states generated, (\d+) distinct states found/.exec(
    stdout,
  );
  assert.ok(counts, "Missing TLC counts");
  return {
    status: "complete",
    states: Number(counts[2]),
    generated: Number(counts[1]),
  };
};
