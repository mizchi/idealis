import { test } from "vite-plus/test";
import { registerWasmHostContract } from "./contract.js";

registerWasmHostContract(test);
