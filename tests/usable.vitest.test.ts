import { test } from "vite-plus/test";
import { registerUsableContract } from "./usable-contract.js";

registerUsableContract((name, run) => {
  test(name, run);
});
