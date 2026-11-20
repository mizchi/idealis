import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  apalacheModule,
  loadTlcStandardModules,
  standardModulesDirectory,
} from "../../evals/experiments/tlc-differential/standard-modules.js";

test("standard module loading verifies the existing fold fixture's exact digest", async () => {
  const loaded = await loadTlcStandardModules();
  assert.equal(
    loaded.modules[0]?.sha256,
    "a9115b404e72c58fd085f81bc59cc918f8adac49feabfb57f8e285bc7ab5cfac",
  );
  assert.ok(loaded.modules[0]?.path.endsWith("Apalache.tla"));
});

test("missing and corrupted standard modules are rejected before checking", async () => {
  const directory = await mkdtemp(join(tmpdir(), "idealis-module-"));
  try {
    await assert.rejects(loadTlcStandardModules(directory), /ENOENT/);
    const source = await readFile(
      join(standardModulesDirectory, apalacheModule.file),
    );
    await writeFile(join(directory, apalacheModule.file), source);
    await loadTlcStandardModules(directory);
    await writeFile(
      join(directory, apalacheModule.file),
      "---- MODULE Apalache ----\n====",
    );
    await assert.rejects(
      loadTlcStandardModules(directory),
      /pinned TLC module changed/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
