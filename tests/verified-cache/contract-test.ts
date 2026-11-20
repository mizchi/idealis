import { test } from "node:test";
import { registerCheckCacheContract } from "../check-cache-contract.js";

registerCheckCacheContract((name, run) => test(name, run));
