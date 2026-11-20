import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyResult } from "../../../packages/idealis/src/safety.js";
import {
  checkTlaSource,
  compileTlaSource,
} from "../../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { traceInput } from "../collections-temporal/tlc.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { classifyTlc, differences } from "../tlc-differential/compare.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";
import { bounds, config, names } from "./cases.js";
import { runTlc } from "./tlc.js";

const directory = process.argv[2];
const java = process.argv[3];
const tlcJar = process.argv[4];
const apalacheJar = process.argv[5];
const saved = process.argv[6];
assert.ok(directory && java && tlcJar && apalacheJar);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const generated = "fixtures/quint-generated/generated";
const base = await fingerprint();
const paths = execFileSync("git", [
  "ls-files",
  "-z",
  "--cached",
  "--others",
  "--exclude-standard",
  "--",
  "evals/experiments/quint-generated",
  "tests/quint-generated",
  "fixtures/quint-generated",
  "evals/plans/0044-quint-generated.md",
  "evals/plans/quint-sharing-regression.md",
])
  .toString()
  .split("\0")
  .filter(Boolean);
const sources = [
  ...base.sources,
  ...(await Promise.all(
    paths.map(async (path) => ({
      path,
      sha256: digest(await readFile(path)),
    })),
  )),
].sort((left, right) => left.path.localeCompare(right.path));
const sourceVersion = digest(JSON.stringify(sources));
const binaries = await Promise.all(
  [
    java,
    tlcJar,
    apalacheJar,
    "crates/idealis-core/target/release/idealis-core",
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const previous: unknown = saved
  ? JSON.parse(await readFile(join(saved, "records.json"), "utf8"))
  : null;
if (saved) {
  const completion: unknown = JSON.parse(
    await readFile(join(saved, "result.json"), "utf8"),
  );
  assert.ok(isRecord(completion) && completion.status === "complete");
  assert.ok(Array.isArray(completion.files));
  assert.deepEqual(
    completion.files
      .map((file: unknown) => {
        assert.ok(isRecord(file));
        return file.path;
      })
      .sort(),
    ["manifest.json", "oracle.json", "records.json"],
  );
  for (const file of completion.files) {
    assert.ok(isRecord(file) && typeof file.path === "string");
    assert.equal(digest(await readFile(join(saved, file.path))), file.sha256);
  }
  assert.equal(completion.sourceVersion, sourceVersion);
  const manifest: unknown = JSON.parse(
    await readFile(join(saved, "manifest.json"), "utf8"),
  );
  assert.ok(isRecord(manifest));
  assert.equal(manifest.sourceVersion, sourceVersion);
  assert.deepEqual(manifest.binaries, binaries);
}
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "quint-sharing-regression",
      irVersions: [1, 2],
      sourceVersion,
      sources,
      binaries,
      bounds,
      parent: saved ?? null,
      commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
      dirty: execFileSync("git", ["status", "--porcelain"]).toString(),
      generation: JSON.parse(
        await readFile(join(generated, "manifest.json"), "utf8"),
      ),
      generatedSourceRewritten: false,
      performanceMeasurement: false,
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const core = await createPortableWasm(await readFile(binaries[4]?.path ?? ""));
const native = startNative();
const records = [];
const oracle = [];
try {
  for (const name of names) {
    const source = await readFile(join(generated, `${name}.tla`), "utf8");
    for (const property of ["safety", "broken"] as const) {
      const input = { source, config: config(property) };
      const caseDirectory = join(directory, `${name}-${property}`);
      await mkdir(caseDirectory);
      const reference = await runTlc(
        java,
        tlcJar,
        apalacheJar,
        join(caseDirectory, "model"),
        name,
        source,
        input.config,
      );
      const expected =
        property === "safety" ? "complete" : "invariant-violation";
      assert.equal(classifyTlc(reference), expected, reference.log);
      if (property === "broken")
        assert.match(reference.log, /Invariant broken is violated/);
      for (const irVersion of [1, 2] as const) {
        const selected = { ...input, irVersion };
        const variantDirectory = join(caseDirectory, `v${irVersion}`);
        await mkdir(variantDirectory);
        const nativeResponse = await native.request({
          version: 1,
          request: { kind: "check-tla", ...selected, bounds },
        });
        await writeFile(
          join(variantDirectory, "native.json"),
          JSON.stringify(nativeResponse, null, 2),
          { flag: "wx" },
        );
        assert.ok(isRecord(nativeResponse));
        assert.equal(
          nativeResponse.kind,
          "safety",
          JSON.stringify(nativeResponse),
        );
        const result = checkTlaSource(core, selected, bounds);
        assert.deepEqual(decodeSafetyResult(nativeResponse.result), result);
        assert.deepEqual(
          differences({ result }, reference),
          [],
          `${name}-${property}`,
        );
        if (result.violation) {
          const wrapper = `---- MODULE Witness ----\nEXTENDS ${name}\nInit == q_init\nNext == q_step\n====`;
          const forced = traceInput(
            { source: wrapper, config: input.config },
            compileTlaSource(core, selected),
            result,
          );
          const witness = await runTlc(
            java,
            tlcJar,
            apalacheJar,
            join(variantDirectory, "witness"),
            "Witness",
            forced.source,
            forced.config,
            { [name]: source },
          );
          assert.equal(classifyTlc(witness), "complete", witness.log);
          assert.equal(witness.states, result.violation.trace.length);
        }
        records.push({ name, property, irVersion, result });
        console.log(
          `${name}-${property} v${irVersion}: ${result.status}, ${result.states} states, ${result.generated} generated`,
        );
      }
      oracle.push({
        name,
        property,
        status: classifyTlc(reference),
        states: reference.states,
        generated: reference.generated,
        tlaSha256: digest(source),
        configSha256: digest(input.config),
        logSha256: digest(reference.log),
      });
    }
  }
  assert.equal(records.length, names.length * 4);
  if (saved) assert.deepEqual(records, previous);
  for (const source of sources)
    assert.equal(
      digest(await readFile(source.path)),
      source.sha256,
      source.path,
    );
  await writeFile(
    join(directory, "records.json"),
    JSON.stringify(records, null, 2),
    { flag: "wx" },
  );
  await writeFile(
    join(directory, "oracle.json"),
    JSON.stringify(oracle, null, 2),
    { flag: "wx" },
  );
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      {
        status: "complete",
        sourceVersion,
        cases: records.length,
        nativeWasm: true,
        tlc: true,
        witnesses: records.filter((entry) => entry.result.violation !== null)
          .length,
        performanceMeasurement: false,
        files: await Promise.all(
          ["manifest.json", "records.json", "oracle.json"].map(
            async (path) => ({
              path,
              sha256: digest(await readFile(join(directory, path))),
            }),
          ),
        ),
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "quint-generated", status: "complete", sourceVersion })}\n`,
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify({ message: String(error), records }, null, 2),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "quint-generated", status: "error", sourceVersion })}\n`,
  );
  throw error;
} finally {
  await native.close();
}
