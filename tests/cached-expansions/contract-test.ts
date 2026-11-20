import { test } from "node:test";
import { registerExpansionContract } from "../expansions-contract.js";

registerExpansionContract((name, run) => test(name, run));
