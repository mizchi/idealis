import {
  createTestingRuntime,
  type TestingRequest,
} from "../../../packages/idealis/src/testing.js";
import { processClient } from "../smt-subset/clients.js";

export const nativePath = "crates/idealis-cli/target/release/idealis-cli";
export const wasmPath =
  "crates/idealis-connect/target/wasm32-unknown-unknown/release/idealis_connect.wasm";
export const createNative = () => {
  const client = processClient(nativePath, [], true);
  const exchanges: { request: TestingRequest; response: unknown }[] = [];
  return {
    runtime: createTestingRuntime(async (request) => {
      const response: unknown = JSON.parse(
        await client.request(JSON.stringify({ version: 1, request })),
      );
      exchanges.push({ request, response });
      return response;
    }),
    exchanges,
    close: client.close,
  };
};
