#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import { checkModel } from "../src/checker.js";
import { createFiniteModel, parseFiniteModel } from "../src/finite.js";
import {
  checkFiniteHistories,
  createPortableWasm,
  decodePortableTrace,
} from "../src/portable.js";

const command = process.argv[2];
const modelPath = process.argv[3];
if (!modelPath || (command !== "check" && command !== "model"))
  throw new Error(
    "Usage: idealis check <model.json> <history.json> | idealis model <model.json> | idealis tlc [options] spec | idealis tla [-s] [-l] spec...",
  );
const model = parseFiniteModel(JSON.parse(await readFile(modelPath, "utf8")));
if (command === "model") {
  const result = checkModel(createFiniteModel(model), {
    maxDepth: model.states.length,
    maxStates: Math.max(1, model.states.length),
  });
  console.log(
    JSON.stringify({
      status: result.status,
      states: result.states,
      edges: result.edges,
      violation: result.violation,
    }),
  );
  process.exitCode =
    result.status === "complete" ? 0 : result.status === "violation" ? 1 : 2;
} else {
  const historyPath = process.argv[4];
  if (!historyPath) throw new Error("A history file is required");
  const trace = decodePortableTrace(await readFile(historyPath, "utf8"));
  const core = await createPortableWasm(
    await readFile(new URL(import.meta.resolve("@mizchi/idealis/core.wasm"))),
  );
  const result = checkFiniteHistories(core, model, [trace])[0];
  if (!result) throw new Error("Missing check result");
  console.log(JSON.stringify(result));
  process.exitCode =
    result.status === "matched" ? 0 : result.status === "mismatch" ? 1 : 2;
}
