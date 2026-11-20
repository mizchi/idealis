import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { check } from "tla-ts";

const path = process.argv[2];
assert.ok(path);
const input = JSON.parse(await readFile(path, "utf8"));
const result = await check({
  source: input.source,
  config: input.config,
  version: "1.7.4",
  workers: 1,
  livenessCheck: "final",
});
console.log(
  JSON.stringify({
    status: result.status === "success" ? "complete" : result.status,
    states: result.distinctStates,
    generated: result.generatedStates,
  }),
);
