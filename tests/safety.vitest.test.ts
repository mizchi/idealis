import { test } from "vite-plus/test";
import { registerSafetyContract } from "./safety-contract.js";

registerSafetyContract((name, run) => {
  test(name, run);
});
