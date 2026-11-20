import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { AlloyInstanceCheck } from "../../../packages/idealis/src/alloy.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { pin } from "../alloy-subset/corpus.js";
import type { Case } from "./corpus.js";

export const reference = async (
  input: Case,
  result: AlloyInstanceCheck,
  java: string,
  jar: string,
  directory: string,
) => {
  await mkdir(directory);
  const path = join(directory, "model.als");
  await writeFile(
    path,
    `${input.input.source}\nfact Observed { ${pin(input.instance)} }\n`,
    { flag: "wx" },
  );
  const run = spawnSync(
    java,
    [
      "-jar",
      jar,
      "exec",
      "--quiet",
      "--command",
      "*",
      "--solver",
      "sat4j",
      "--type",
      "json",
      "--output",
      resolve(directory, "output"),
      resolve(path),
    ],
    { encoding: "utf8" },
  );
  await writeFile(join(directory, "stdout.log"), run.stdout ?? "", {
    flag: "wx",
  });
  await writeFile(join(directory, "stderr.log"), run.stderr ?? "", {
    flag: "wx",
  });
  if (run.error) throw run.error;
  assert.equal(run.status, 0, `${input.name}: ${run.stderr}`);
  const receipt: unknown = JSON.parse(
    await readFile(join(directory, "output/receipt.json"), "utf8"),
  );
  assert.ok(isRecord(receipt) && isRecord(receipt.commands));
  const commands = receipt.commands;
  const query = commands.Query;
  assert.ok(isRecord(query));
  assert.equal(query.type, result.command.kind);
  const sat = (name: string): boolean => {
    const value = commands[name];
    assert.ok(isRecord(value));
    return Array.isArray(value.solution) && value.solution.length > 0;
  };
  const feasible = sat("Feasible");
  const holds = result.command.kind === "run" ? sat("Query") : !sat("Query");
  const status = feasible ? (holds ? "matched" : "mismatch") : "invalid";
  assert.equal(result.status, status, input.name);
  return { status, feasible, querySat: sat("Query") };
};
