import { test } from "vite-plus/test";
import { registerSourceContract } from "./source-contract.js";

registerSourceContract((name, run) => {
  test(name, run);
});
