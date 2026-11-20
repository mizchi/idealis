import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { config, names } from "./cases.js";

const directory = process.argv[2];
const endpoint = process.argv[3];
const apalacheJar = process.argv[4];
assert.ok(directory && endpoint && apalacheJar);
await mkdir(directory);
const records = [];
for (const name of names) {
  const input = `fixtures/quint-generated/${name}.qnt`;
  const args: string[] = [
    "dlx",
    "@informalsystems/quint@0.33.1",
    "compile",
    input,
    "--init=init",
    "--step=step",
    "--invariant=safety,broken",
    "--target=tlaplus",
    `--server-endpoint=${endpoint}`,
    "--apalache-version=0.63.0",
  ];
  const result = spawnSync("pnpm", args, {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  await writeFile(join(directory, `${name}.stdout`), result.stdout, {
    flag: "wx",
  });
  await writeFile(join(directory, `${name}.stderr`), result.stderr, {
    flag: "wx",
  });
  assert.equal(result.status, 0, `${name}: ${result.stderr}`);
  assert.match(result.stdout, /^-+ MODULE /);
  const source = await readFile(input);
  await writeFile(join(directory, `${name}.qnt`), source, { flag: "wx" });
  await writeFile(join(directory, `${name}.tla`), result.stdout, {
    flag: "wx",
  });
  for (const property of ["safety", "broken"] as const)
    await writeFile(
      join(directory, `${name}-${property}.cfg`),
      config(property),
      {
        flag: "wx",
      },
    );
  records.push({
    name,
    command: ["pnpm", ...args],
    exit: result.status,
    quintSha256: createHash("sha256").update(source).digest("hex"),
    tlaSha256: createHash("sha256").update(result.stdout).digest("hex"),
  });
}
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      quint: "0.33.1",
      apalache: "0.63.0",
      apalacheSha256: createHash("sha256")
        .update(await readFile(apalacheJar))
        .digest("hex"),
      node: process.version,
      records,
    },
    null,
    2,
  ),
  { flag: "wx" },
);
