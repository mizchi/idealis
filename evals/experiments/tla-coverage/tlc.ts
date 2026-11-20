import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { runTlc } from "../collections-temporal/tlc.js";
import { digest } from "../usable/provenance.js";

const [directory, java, jar] = process.argv.slice(2);
assert.ok(directory && java && jar);
const path = "crates/rtla-compiler/tests/frontend-completion.json";
const bytes = await readFile(path);
const cases: unknown = JSON.parse(bytes.toString());
assert.ok(Array.isArray(cases));
await mkdir(directory);
const save = (name: string, value: unknown) =>
  writeFile(join(directory, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  plan: "0070-tla-coverage",
  input: { path, sha256: digest(bytes) },
  jarSha256: digest(await readFile(jar)),
  standardModuleSha256: digest(
    await readFile(".artifacts/tlc/standard-modules/Apalache.tla"),
  ),
  scope:
    "Independent TLC execution controls; terminating cases only; no native comparison in this run.",
});
const results = [];
for (const [index, input] of cases.entries()) {
  assert.ok(
    isRecord(input) &&
      typeof input.source === "string" &&
      typeof input.config === "string",
  );
  const modules: Record<string, string> = {};
  if (input.modules !== undefined) {
    assert.ok(isRecord(input.modules));
    for (const [name, source] of Object.entries(input.modules)) {
      assert.equal(typeof source, "string");
      assert.ok(typeof source === "string");
      modules[name] = source;
    }
  }
  const result = await runTlc(
    java,
    jar,
    join(directory, String(index)),
    { source: input.source, config: input.config, modules },
    "Completion",
    { libraryPath: ".artifacts/tlc/standard-modules" },
  );
  results.push({
    name: input.name,
    family: input.family,
    expected: { status: input.status, states: input.states },
    ...result,
  });
}
await save("results.json", results);
assert.equal(
  digest(await readFile(path)),
  digest(bytes),
  "Fixed inputs changed during reference execution",
);
const differences = results.filter(
  (result) =>
    result.status !== result.expected.status ||
    result.states !== result.expected.states,
);
await save("summary.json", {
  cases: results.length,
  differences: differences.map((result) => ({
    name: result.name,
    family: result.family,
    status: result.status,
    states: result.states,
    expected: result.expected,
  })),
});
assert.equal(differences.length, 0, "TLC controls differ; inspect raw logs");
