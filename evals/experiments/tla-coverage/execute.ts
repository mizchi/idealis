import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type {
  PortableCommand,
  TlaSourceInput,
} from "../../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { runTlc } from "../collections-temporal/tlc.js";
import { classifyTlc } from "../tlc-differential/compare.js";
import { differential } from "../tlc-differential/differential.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";

interface Case extends TlaSourceInput {
  readonly name: string;
  readonly family: string;
  readonly versions: readonly (1 | 2)[];
  readonly status: string;
  readonly states: number | null;
}
const [directory, snapshot, java, jar] = process.argv.slice(2);
assert.ok(directory && snapshot && java && jar);
await mkdir(directory);
const paths = [
  "crates/rtla-compiler/tests/frontend-completion.json",
  "evals/experiments/tla-coverage/execute.ts",
  "evals/experiments/tlc-differential/differential.ts",
  "evals/experiments/collections-temporal/tlc.ts",
  ".artifacts/tlc/standard-modules/Apalache.tla",
  join(snapshot, "idealis-core"),
  join(snapshot, "idealis-core.wasm"),
  jar,
];
const snapshotSources: readonly { path: string; sha256: string }[] = JSON.parse(
  await readFile(join(snapshot, "manifest.json"), "utf8"),
).sources;
const sources = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const inputs: readonly Case[] = JSON.parse(
  await readFile(paths[0] ?? "", "utf8"),
);
const save = (name: string, value: unknown) =>
  writeFile(join(directory, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
await save("manifest.json", {
  plan: "0070-tla-coverage",
  snapshot: JSON.parse(await readFile(join(snapshot, "manifest.json"), "utf8")),
  sources,
  inputs,
  scope:
    "Native/Wasm exact compile and source checks; independent TLC verdict/counts and witness replay; work cut is inconclusive. Rejected inputs have a separate outcome.",
});
const wasm = await createPortableWasm(
  await readFile(join(snapshot, "idealis-core.wasm")),
);
const native = startNative(join(snapshot, "idealis-core"));
const request = (command: PortableCommand) => {
  try {
    return wasm.request(command);
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error" as const, code: error.code, message: error.message };
  }
};
const options = { libraryPath: ".artifacts/tlc/standard-modules" };
const bounds = {
  maxDepth: 100,
  maxStates: 1000,
  maxWork: 1000000,
  maxTemporalNodes: 10000,
};
const results = [];
let index = 0;
try {
  for (const original of inputs) {
    for (const irVersion of original.versions) {
      const input = {
        ...original,
        source: original.source.replace(
          "MODULE Completion",
          "MODULE Differential",
        ),
        irVersion,
      };
      const command: PortableCommand = {
        version: 1,
        request: {
          kind: "compile-tla",
          source: input.source,
          config: input.config,
          ...(input.modules ? { modules: input.modules } : {}),
          irVersion,
        },
      };
      const compiled = request(command);
      assert.deepEqual(
        await native.request(command),
        compiled,
        `${input.name}/${irVersion}: compile native/Wasm`,
      );
      const folder = String(index++);
      await mkdir(join(directory, folder));
      if (input.status === "error") {
        assert.ok(isRecord(compiled) && compiled.kind === "error", input.name);
        const reference = await runTlc(
          java,
          jar,
          join(directory, folder, "reference"),
          input,
          "Differential",
          options,
        );
        assert.equal(classifyTlc(reference), "input-error", input.name);
        const result = {
          name: input.name,
          irVersion,
          outcome: "rejected",
          compiled,
          reference: classifyTlc(reference),
        };
        await save(`${folder}/result.json`, result);
        results.push(result);
        continue;
      }
      assert.ok(
        isRecord(compiled) && compiled.kind === "compiled",
        `${input.name}: ${JSON.stringify(compiled)}`,
      );
      const sourceCommand: PortableCommand = {
        version: 1,
        request: {
          kind: "check-tla",
          source: input.source,
          config: input.config,
          ...(input.modules ? { modules: input.modules } : {}),
          irVersion,
          bounds,
        },
      };
      const checked = request(sourceCommand);
      assert.deepEqual(
        await native.request(sourceCommand),
        checked,
        `${input.name}/${irVersion}: check source native/Wasm`,
      );
      const report = await differential(
        { ...input, family: "control" },
        wasm,
        native,
        java,
        jar,
        join(directory, folder),
        (entry) => compileTlaSource(wasm, { ...input, ...entry }),
        options,
      );
      const cut: PortableCommand = {
        version: 1,
        request: {
          kind: "check-safety",
          program: compileTlaSource(wasm, input),
          bounds: { ...bounds, maxWork: 1 },
        },
      };
      const partial = request(cut);
      assert.deepEqual(await native.request(cut), partial);
      assert.ok(
        isRecord(partial) &&
          partial.kind === "safety" &&
          isRecord(partial.result),
      );
      assert.equal(partial.result.status, "bounded");
      assert.equal(partial.result.reason, "work");
      assert.equal(partial.result.work, 1);
      const result = {
        ...report,
        name: input.name,
        irVersion,
        outcome: report.differences.length ? "mismatch" : "matched",
        checked,
        cut: partial,
      };
      await save(`${folder}/result.json`, result);
      results.push(result);
    }
  }
  const families = new Set<string>();
  for (const input of inputs) {
    if (input.status !== "complete" || families.has(input.family)) continue;
    families.add(input.family);
    const irVersion = input.versions.includes(2) ? (2 as const) : (1 as const);
    const fault = {
      ...input,
      name: `${input.name}-fault`,
      family: "control" as const,
      irVersion,
      source: input.source
        .replace("MODULE Completion", "MODULE Differential")
        .replace(/\nSafe ==[\s\S]*?\n====\s*$/, "\nSafe == x # x\n===="),
      config: input.config.replace("PROPERTY Safe", "INVARIANT Safe"),
    };
    assert.notEqual(fault.source, input.source);
    const folder = String(index++);
    await mkdir(join(directory, folder));
    const report = await differential(
      fault,
      wasm,
      native,
      java,
      jar,
      join(directory, folder),
      (entry) => compileTlaSource(wasm, { ...fault, ...entry }),
      options,
    );
    assert.equal(report.ours.result?.status, "invariant-violation", fault.name);
    assert.ok(report.witness, fault.name);
    const result = {
      ...report,
      name: fault.name,
      outcome: report.differences.length ? "mismatch" : "matched",
    };
    await save(`${folder}/result.json`, result);
    results.push(result);
  }
} finally {
  await native.close();
}
await save("results.json", results);
for (const source of [...sources, ...snapshotSources])
  assert.equal(digest(await readFile(source.path)), source.sha256, source.path);
const differences = results.filter((result) => result.outcome === "mismatch");
await save("summary.json", {
  cases: results.length,
  matched: results.filter((result) => result.outcome === "matched").length,
  rejected: results.filter((result) => result.outcome === "rejected").length,
  differences,
});
assert.equal(differences.length, 0, "Inspect saved differential reports");
