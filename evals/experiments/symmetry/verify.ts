import assert from "node:assert/strict";
import {
  appendFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import { digest } from "../usable/provenance.js";
import { symmetryFingerprint } from "./provenance.js";

const directory = process.argv[2];
assert.ok(directory);
await mkdir(directory);
const files: { path: string; sha256: string }[] = [];
const save = async (path: string, bytes: string | Buffer) => {
  await writeFile(join(directory, path), bytes, { flag: "wx" });
  files.push({ path, sha256: digest(bytes) });
};
const sources = await symmetryFingerprint();
const frozen: { sources: { path: string; sha256: string }[] } = JSON.parse(
  await readFile(".artifacts/0061-final/manifest.json", "utf8"),
);
for (const source of frozen.sources)
  assert.equal(digest(await readFile(source.path)), source.sha256, source.path);
await save("sources.json", `${JSON.stringify(sources, null, 2)}\n`);
await save(
  "final-source.tar.gz",
  gzipSync(await readFile(".artifacts/0061-final/source.tar")),
);
for (const stage of ["stage2", "stage3", "stage5"])
  await save(
    `${stage}-source.tar.gz`,
    gzipSync(await readFile(`.artifacts/0061-${stage}/source.tar`)),
  );
const runs = [];
const ledger = await readFile("evals/runs/ledger.jsonl", "utf8");
for (const name of (await readdir("evals/runs"))
  .filter(
    (name) => name.startsWith("0061-") && name !== directory.split("/").at(-1),
  )
  .sort()) {
  const entries = await readdir(join("evals/runs", name));
  const result = entries.includes("result.json")
    ? JSON.parse(
        await readFile(join("evals/runs", name, "result.json"), "utf8"),
      )
    : null;
  if (result) {
    assert.equal(result.status, "complete", name);
    if (result.mismatches !== undefined)
      assert.equal(
        Array.isArray(result.mismatches)
          ? result.mismatches.length
          : result.mismatches,
        0,
        name,
      );
    for (const file of result.files ?? [])
      assert.equal(
        digest(await readFile(join("evals/runs", name, file.path))),
        file.sha256,
        `${name}/${file.path}`,
      );
  }
  const run = {
    runId: name,
    status: result ? "complete" : "error",
    resultSha256: result
      ? digest(await readFile(join("evals/runs", name, "result.json")))
      : null,
  };
  runs.push(run);
  if (!ledger.includes(`"runId":"${name}"`))
    await appendFile(
      "evals/runs/ledger.jsonl",
      `${JSON.stringify({ ...run, kind: "safety-tuning-0061", plan: "0061-symmetry-next" })}\n`,
    );
}
for (const name of [
  "0061-final-criterion",
  "0061-final-tlc",
  "0061-final-actions-tlc",
  "0061-final-por-tlc",
  "0061-final-parity",
  "0061-final-existing-tlc",
])
  assert.ok(
    runs.some((run) => run.runId === name && run.status === "complete"),
    name,
  );
for (const name of (await readdir(".artifacts"))
  .filter(
    (name) =>
      name.startsWith("0061-") &&
      name.endsWith(".log") &&
      !name.includes("support-sync"),
  )
  .sort())
  await save(`${name}.gz`, gzipSync(await readFile(join(".artifacts", name))));
const binaries = await Promise.all(
  ["idealis-core", "idealis_core.wasm", "safety-bench", "symmetry-bench"].map(
    async (name) => ({
      name,
      sha256: digest(await readFile(`.artifacts/0061-final/${name}`)),
    }),
  ),
);
assert.equal(
  digest(await readFile("crates/idealis-core/target/release/idealis-core")),
  binaries.find((binary) => binary.name === "idealis-core")?.sha256,
);
assert.equal(
  digest(
    await readFile(
      "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
    ),
  ),
  binaries.find((binary) => binary.name === "idealis_core.wasm")?.sha256,
);
const result = {
  status: "complete",
  plan: "0061-symmetry-next",
  sourceVersion: sources.sourceVersion,
  frozenSourceFiles: frozen.sources.length,
  binaries,
  runs,
  files,
  decisions: {
    recordsAndNodeSets: "adopted",
    actionOrbits: "opt-in",
    symmetryParallel: "frontier at least 64",
    directInstructions: "rejected",
    partialOrder: "opt-in invisible monotone independent Boolean flags",
  },
};
await writeFile(
  join(directory, "result.json"),
  `${JSON.stringify(result, null, 2)}\n`,
  { flag: "wx" },
);
await appendFile(
  "evals/runs/ledger.jsonl",
  `${JSON.stringify({ runId: directory.split("/").at(-1), kind: "safety-tuning-verification", status: "complete", sourceVersion: sources.sourceVersion })}\n`,
);
console.log(
  `Verified ${runs.length} runs, ${frozen.sources.length} frozen source files and ${binaries.length} binary hashes`,
);
