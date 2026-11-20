import { test } from "vite-plus/test";
import { registerBackendContract } from "./backend-contract.js";

registerBackendContract((name, run) => {
  test(name, run);
});
