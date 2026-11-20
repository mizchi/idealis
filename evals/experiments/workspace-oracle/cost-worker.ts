import { createInterface } from "node:readline";
import { isDeepStrictEqual } from "node:util";
import { compareOracle } from "./node.js";

const mode = process.argv[2];
process.env.IDEALIS_ORACLE_BACKEND = "wasm";
if (mode !== "legacy" && mode !== "oracle")
  throw new Error("Expected legacy/oracle mode");
for await (const line of createInterface({ input: process.stdin })) {
  const input = JSON.parse(line);
  const status =
    mode === "legacy"
      ? isDeepStrictEqual(input.actual, input.expected)
        ? "matched"
        : "mismatch"
      : compareOracle(input.actual, input.expected).status;
  console.log(JSON.stringify({ status }));
}
