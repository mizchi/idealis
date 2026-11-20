import {
  fixtures,
  observationMatrix,
  requestFor,
} from "../../evals/experiments/roracle-native/fixtures.js";
import { instantiate } from "../../evals/experiments/roracle-wit/generated/oracle.js";
import { witClient } from "../../evals/experiments/roracle-wit/wit-client.js";
import { createOracle } from "../../evals/experiments/workspace-oracle/wasm.js";
import { equalValues } from "../../packages/idealis/src/trace.js";

export const run = async () => {
  const bytes = await (await fetch("/roracle.wasm")).arrayBuffer();
  const oracle = createOracle(new Uint8Array(bytes));
  const core = new WebAssembly.Module(
    await (await fetch("/oracle.core.wasm")).arrayBuffer(),
  );
  const root = instantiate(() => core, {});
  const clients = [
    witClient(root, "wit-record"),
    witClient(root, "wit-buffer"),
  ];
  let checked = 0;
  for (const input of [
    ...fixtures.map((fixture) => fixture.input),
    ...observationMatrix.flatMap((reference) =>
      observationMatrix.map((actual) => requestFor(reference, actual)),
    ),
  ]) {
    const expected = oracle.request(input);
    for (const client of clients) {
      if (!equalValues(client.request(input), expected))
        throw new Error("Browser WIT report differs");
      checked++;
    }
  }
  root._util.stop();
  return { status: "matched", reports: checked };
};
