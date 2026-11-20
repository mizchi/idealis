import { processClient } from "../smt-subset/clients.js";
import { decode, type Request } from "./wire.js";

export const nativePath = "crates/ralloy/target/release/ralloy";
export const wasmPath =
  "crates/ralloy/target/wasm32-unknown-unknown/release/ralloy.wasm";
export const maxWork = 2_000_000;
export const createNative = () => {
  const client = processClient(nativePath, [], true);
  return {
    request: async (request: Request) =>
      decode(
        JSON.parse(
          await client.request(JSON.stringify({ version: 1, request })),
        ),
      ),
    close: client.close,
  };
};
