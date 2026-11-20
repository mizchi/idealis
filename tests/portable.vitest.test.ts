import { test } from "vite-plus/test";
import { registerPortableContract } from "./portable-contract.js";

registerPortableContract((name, run) => {
  test(name, run);
});
