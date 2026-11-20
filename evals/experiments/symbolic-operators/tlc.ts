import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { runTlc } from "../collections-temporal/tlc.js";
import { classifyTlc } from "../tlc-differential/compare.js";
import { differential } from "../tlc-differential/differential.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";
import { models } from "./models.js";

const [directory, java, jar] = process.argv.slice(2);
assert.ok(directory && java && jar);
await mkdir(directory);
const save = (path: string, value: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const paths = execFileSync("git", [
  "ls-files",
  "--cached",
  "--others",
  "--exclude-standard",
  "--",
  "crates/rtla",
  "crates/rtla-compiler",
  "evals/experiments/symbolic-operators",
  "tests/symbolic-operators",
  "evals/plans/0069-symbolic-operators.md",
])
  .toString()
  .trim()
  .split("\n")
  .sort();
const sources = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const binaries = await Promise.all(
  [
    "crates/idealis-core/target/release/idealis-core",
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
    jar,
  ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
await save("manifest.json", {
  plan: "0069-symbolic-operators",
  sources,
  binaries,
  inputs: models,
  comparison:
    "Valid finite models use the existing TLC differential and trace replay. Builtin conflicts and imported ambiguities are recorded separately from matched model checks.",
});
const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const native = startNative();
const records = [];
try {
  for (const model of models) {
    await mkdir(join(directory, model.name));
    if (
      model.expected === "builtin-conflict" ||
      model.expected === "import-ambiguity"
    ) {
      const reference = await runTlc(
        java,
        jar,
        join(directory, model.name, "reference"),
        model,
        "Differential",
      );
      const actual = [];
      for (const irVersion of [1, 2] as const) {
        const response = await native.request({
          version: 1,
          request: {
            kind: "compile-tla",
            source: model.source,
            config: model.config,
            irVersion,
          },
        });
        await save(join(model.name, `native-v${irVersion}.json`), response);
        assert.ok(
          isRecord(response) &&
            response.kind === "error" &&
            response.code === "protocol" &&
            typeof response.message === "string" &&
            /builtin or imported symbol/.test(response.message),
          model.name,
        );
        assert.throws(
          () => compileTlaSource(core, { ...model, irVersion }),
          /builtin or imported symbol/,
        );
        actual.push({ irVersion, response });
      }
      const classification = classifyTlc(reference);
      await save(join(model.name, "result.json"), {
        input: model,
        classification,
        reference,
        actual,
      });
      assert.equal(classification, "input-error", model.name);
      records.push({
        name: model.name,
        kind: model.expected,
        classification,
        actual,
      });
    } else {
      for (const irVersion of [1, 2] as const) {
        const path = join(model.name, `v${irVersion}`);
        await mkdir(join(directory, path));
        const report = await differential(
          { ...model, irVersion },
          core,
          native,
          java,
          jar,
          join(directory, path),
          (input) => compileTlaSource(core, input),
        );
        await save(join(path, "report.json"), report);
        assert.equal(report.reference.status, model.expected, model.name);
        assert.deepEqual(report.differences, [], model.name);
        records.push({
          name: model.name,
          kind: "matched-model",
          irVersion,
          report,
        });
      }
    }
  }
  for (const file of [...sources, ...binaries])
    assert.equal(digest(await readFile(file.path)), file.sha256, file.path);
  await save("result.json", {
    status: "complete",
    compatibility: "partial",
    models: models.length,
    records,
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "symbolic-tlc-differential", status: "complete", compatibility: "partial" })}\n`,
  );
} catch (error) {
  await save("error.json", {
    status: "error",
    message: String(error),
    records,
  });
  throw error;
} finally {
  await native.close();
}
