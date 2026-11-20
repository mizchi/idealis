import { test } from "node:test";
import { registerWasmHostContract } from "./contract.js";

registerWasmHostContract(test);
