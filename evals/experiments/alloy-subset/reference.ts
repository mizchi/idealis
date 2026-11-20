import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { type Case, pin, source } from "./corpus.js";
import type { Analysis } from "./wire.js";

export const reference = async (
  input: Case,
  result: Analysis,
  java: string,
  jar: string,
  directory: string,
) => {
  await mkdir(directory);
  const query = input.query.trim() || "no none";
  const pinned = result.instance
    ? `Pinned: run { (${input.kind === "check" ? `not (${query})` : query}) and (${pin(result.instance)}) } ${input.scope}\n`
    : "";
  const path = join(directory, "model.als");
  await writeFile(path, source(input) + pinned, { flag: "wx" });
  const output = spawnSync(
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
  await writeFile(join(directory, "stdout.log"), output.stdout ?? "", {
    flag: "wx",
  });
  await writeFile(join(directory, "stderr.log"), output.stderr ?? "", {
    flag: "wx",
  });
  if (output.error) throw output.error;
  assert.equal(output.status, 0, `${input.name}: ${output.stderr}`);
  const receipt: unknown = JSON.parse(
    await readFile(join(directory, "output/receipt.json"), "utf8"),
  );
  assert.ok(isRecord(receipt) && isRecord(receipt.commands));
  const commands = receipt.commands;
  const sat = (name: string): boolean => {
    const command = commands[name];
    assert.ok(isRecord(command), `${input.name}: missing ${name}`);
    return Array.isArray(command.solution) && command.solution.length > 0;
  };
  const querySat = sat("Query");
  const factsSat = sat("Feasible");
  const expected =
    input.kind === "run"
      ? querySat
        ? "instance"
        : "no-instance"
      : querySat
        ? "counterexample"
        : factsSat
          ? "no-counterexample"
          : "no-instance";
  assert.equal(result.status, expected, input.name);
  if (result.instance)
    assert.equal(
      sat("Pinned"),
      true,
      `${input.name}: pinned instance rejected`,
    );
  return {
    status: expected,
    querySat,
    factsSat,
    pinned: result.instance !== null,
  };
};
