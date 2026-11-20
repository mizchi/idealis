import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import {
  checkSafetyIncremental,
  createSafetyExpansionCache,
} from "../../../packages/idealis/src/safety-expansions.js";
import { checkSafety } from "../../../packages/idealis/src/safety.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import { bounds, config, names } from "../quint-generated/cases.js";
import { createRunDirectory } from "./directory.js";

const [output, saved] = process.argv.slice(2);
assert.ok(output);
await createRunDirectory(output);
const paths = [
  "crates/rtlc/src/expansions.rs",
  "crates/rtlc/src/safety.rs",
  "crates/rtlc/src/safety_values.rs",
  "evals/experiments/quint-generated/cases.ts",
  "crates/rtlc/src/safety_parallel.rs",
  "crates/rtlc/Cargo.lock",
  "crates/idealis-core/src/lib.rs",
  "packages/idealis/src/contracts.ts",
  "packages/idealis/src/portable-wasm.ts",
  "packages/idealis/src/safety-expansions.ts",
  "formal/cache/Cache.lean",
  "formal/cache/ReuseMain.lean",
  "evals/experiments/cached-expansions/run.ts",
  "evals/experiments/cached-expansions/directory.ts",
];
const digest = (body: string | Uint8Array) =>
  createHash("sha256").update(body).digest("hex");
const sources = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const bytes = await readFile(
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
);
const buildDigest = digest(bytes);
const core = await createPortableWasm(bytes);
const models = saved
  ? JSON.parse(await readFile(resolve(saved, "models.json"), "utf8"))
  : await Promise.all(
      names.map(async (name) => ({
        name,
        source: await readFile(
          `fixtures/quint-generated/generated/${name}.tla`,
          "utf8",
        ),
      })),
    );
const manifest = {
  plan: "evals/plans/cached-expansions.md",
  node: process.version,
  buildDigest,
  sources,
  modelsDigest: digest(JSON.stringify(models)),
  saved: saved ?? null,
};
await writeFile(
  resolve(output, "manifest.json"),
  JSON.stringify(manifest, null, 2),
);
await writeFile(
  resolve(output, "models.json"),
  JSON.stringify(models, null, 2),
);
if (saved) {
  const before = JSON.parse(
    await readFile(resolve(saved, "manifest.json"), "utf8"),
  );
  assert.deepEqual(manifest.sources, before.sources);
  assert.equal(buildDigest, before.buildDigest);
  assert.equal(manifest.modelsDigest, before.modelsDigest);
}
const rows = [];
for (const input of models)
  for (const irVersion of [1, 2] as const) {
    const program = compileTlaSource(core, {
      source: input.source,
      config: config("safety"),
      irVersion,
    });
    const options = { enabled: true, buildDigest, maxEntries: 10000 };
    const cold = checkSafetyIncremental(core, program, bounds, {
      ...options,
      cache: createSafetyExpansionCache(),
    });
    const warm = checkSafetyIncremental(core, program, bounds, {
      ...options,
      cache: cold.cache,
    });
    const broken = compileTlaSource(core, {
      source: input.source,
      config: config("broken"),
      irVersion,
    });
    const changed = checkSafetyIncremental(core, broken, bounds, {
      ...options,
      cache: warm.cache,
    });
    assert.equal(changed.result.status, "invariant-violation");
    const baseline = checkSafety(core, broken, bounds);
    assert.deepEqual({ ...changed.result, work: 0 }, { ...baseline, work: 0 });
    assert.equal(warm.result.status, "complete");
    assert.equal(warm.stats.hits, cold.result.states);
    assert.equal(warm.stats.misses, 0);
    rows.push({ name: input.name, irVersion, cold, warm, changed });
  }
await writeFile(
  resolve(output, "observations.json"),
  JSON.stringify(rows, null, 2),
);
const probes = [];
const base =
  "---- MODULE Reuse ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nSafe == x>=0\n====";
for (let bits = 0; bits < 32; bits++) {
  const probe = {
    enabled: !!(bits & 16),
    sameBuild: !!(bits & 1),
    sameNext: !!(bits & 2),
    sameState: !!(bits & 4),
    complete: !!(bits & 8),
  };
  const seed = compileTlaSource(core, {
    source: base,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  });
  const seeded = checkSafetyIncremental(core, seed, bounds, {
    enabled: true,
    cache: createSafetyExpansionCache(),
    buildDigest,
    maxEntries: probe.complete ? 100 : 0,
  });
  const changed = base
    .replace("Init == x=0", probe.sameState ? "Init == x=0" : "Init == x=1")
    .replace(
      "Next == UNCHANGED x",
      probe.sameNext ? "Next == UNCHANGED x" : "Next == x'=x+0",
    );
  const program = compileTlaSource(core, {
    source: changed,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  });
  const result = checkSafetyIncremental(core, program, bounds, {
    enabled: probe.enabled,
    cache: seeded.cache,
    buildDigest: probe.sameBuild ? buildDigest : "f".repeat(64),
    maxEntries: 100,
  });
  assert.equal(result.result.status, "complete");
  const reused = result.stats.hits === 1;
  assert.equal(reused, bits === 31);
  probes.push({ probe, reused, stats: result.stats });
}
await writeFile(
  resolve(output, "reuse-probes.json"),
  JSON.stringify(probes, null, 2),
);
const result = {
  status: "complete",
  modelCases: rows.length,
  warmMisses: rows.reduce((sum, row) => sum + row.warm.stats.misses, 0),
  changedViolations: rows.filter(
    (row) => row.changed.result.status === "invariant-violation",
  ).length,
  guardCases: probes.length,
};
if (saved)
  assert.deepEqual(
    result,
    JSON.parse(await readFile(resolve(saved, "result.json"), "utf8")),
  );
await writeFile(
  resolve(output, "result.json"),
  JSON.stringify(result, null, 2),
);
console.log(result);
