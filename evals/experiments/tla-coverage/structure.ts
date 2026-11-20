import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { digest } from "../usable/provenance.js";
import { normalizeRtla, normalizeSany } from "./normalize.js";

const [audit, directory] = process.argv.slice(2);
assert.ok(audit && directory);
await mkdir(directory);
const load = async (name: string): Promise<unknown> =>
  JSON.parse(await readFile(join(audit, name), "utf8"));
const inputs = await load("inputs.json");
assert.ok(Array.isArray(inputs));
const results = [];
for (const version of ["1.7.4", "1.8.0"]) {
  for (const [index, input] of inputs.entries()) {
    const rtla = await load(`cases/${index}/rtla-${version}.json`);
    const sany = await load(`cases/${index}/sany-${version}-forward.json`);
    assert.ok(isRecord(rtla) && isRecord(sany));
    if (rtla.kind !== "module" || sany.kind !== "accepted") continue;
    let status = "matched";
    let error: string | undefined;
    let expected: unknown;
    let actual: unknown;
    try {
      expected = normalizeSany(sany.tree);
      actual = normalizeRtla(rtla.module);
      assert.deepEqual(actual, expected);
    } catch (failure) {
      status = "mismatch";
      error = failure instanceof Error ? failure.message : String(failure);
    }
    const result = {
      index,
      version,
      name: input.name,
      family: input.family,
      status,
      error,
    };
    if (status === "mismatch")
      await writeFile(
        join(directory, `${index}-${version}.json`),
        JSON.stringify({ ...result, actual, expected }, null, 2),
        { flag: "wx" },
      );
    results.push(result);
  }
}
await writeFile(
  join(directory, "result.json"),
  JSON.stringify(
    {
      audit,
      normalizerSha256: digest(
        await readFile("evals/experiments/tla-coverage/normalize.ts"),
      ),
      results,
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const failures = results.filter((result) => result.status !== "matched");
await writeFile(
  join(directory, "summary.json"),
  JSON.stringify(
    {
      cases: results.length,
      matched: results.length - failures.length,
      failures: failures.map((result) => ({
        ...result,
        error: result.error?.slice(0, 400),
      })),
    },
    null,
    2,
  ),
  { flag: "wx" },
);
assert.equal(failures.length, 0, "Structural AST differences remain");
