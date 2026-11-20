import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { digest } from "../usable/provenance.js";
import { benchmarkCases } from "./cases.js";

const [directory, before, after] = process.argv.slice(2);
assert.ok(directory && before && after);
await mkdir(directory);
const binaries = await Promise.all(
  [before, after].map(async (snapshot) => {
    const build = JSON.parse(
      await readFile(join(snapshot, "build.json"), "utf8"),
    );
    assert.equal(build.status, "complete");
    const entry = build.binaries.find(
      (entry: { path: string }) => entry.path === "semantics-profile",
    );
    assert.ok(entry);
    const binary = resolve(snapshot, entry.path);
    assert.equal(digest(await readFile(binary)), entry.sha256);
    return { snapshot, binary, sha256: entry.sha256 };
  }),
);
const results = benchmarkCases.map((input) => {
  const responses = binaries.map((entry) =>
    JSON.parse(execFileSync(entry.binary, [input.name, "1"]).toString()),
  );
  assert.deepEqual(responses[0], responses[1], input.name);
  return { name: input.name, responses };
});
for (const entry of binaries)
  assert.equal(digest(await readFile(entry.binary)), entry.sha256);
await writeFile(
  join(directory, "result.json"),
  `${JSON.stringify({ plan: "0072-frontend-semantics", binaries, results }, null, 2)}\n`,
  { flag: "wx" },
);
console.log(`${results.length} complete responses, including work, equal`);
