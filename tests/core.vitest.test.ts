import { test } from "vite-plus/test";
import { registerAdvancedContract } from "./advanced-contract.js";
import { registerContract } from "./contract.js";

registerContract(test);
registerAdvancedContract(test);
