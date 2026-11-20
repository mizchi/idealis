import {
  createWasm,
  equalReply,
} from "../../evals/experiments/structured-fuzz/wasm.js";

interface Record {
  readonly report: {
    readonly trace: readonly {
      readonly request: readonly number[];
      readonly response: unknown;
    }[];
  };
}
export const run = async (assetBase = "") => {
  const wasm = await createWasm(
    new Uint8Array(
      await (await fetch(`${assetBase}/testing.wasm`)).arrayBuffer(),
    ),
  );
  const records: readonly Record[] = await (
    await fetch(`${assetBase}/cases.json`)
  ).json();
  let requests = 0;
  const replies = records.map((record) => {
    wasm.request(
      new TextEncoder().encode('{"version":1,"request":{"kind":"cancel"}}'),
    );
    return record.report.trace.map((entry) => {
      const response = wasm.request(Uint8Array.from(entry.request));
      if (!equalReply(entry.response, response))
        throw new Error("Chromium reply differs");
      requests++;
      return response;
    });
  });
  return {
    records: records.length,
    requests,
    imports: wasm.imports,
    isolated: globalThis.crossOriginIsolated ?? false,
    replies,
  };
};
