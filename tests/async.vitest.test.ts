import { test } from "vite-plus/test";
import { registerAsyncContract } from "./async-contract.js";

registerAsyncContract((name, run) => {
  test(name, run);
});
