import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

export const apalacheModule = {
  name: "Apalache",
  file: "Apalache.tla",
  url: "https://raw.githubusercontent.com/apalache-mc/apalache/v0.56.1/src/tla/Apalache.tla",
  sha256: "a9115b404e72c58fd085f81bc59cc918f8adac49feabfb57f8e285bc7ab5cfac",
};
export const standardModulesDirectory = ".artifacts/tlc/standard-modules";

export const loadTlcStandardModules = async (
  directory = standardModulesDirectory,
) => {
  const path = resolve(directory, apalacheModule.file);
  const bytes = await readFile(path);
  assert.equal(
    createHash("sha256").update(bytes).digest("hex"),
    apalacheModule.sha256,
    `${path}: pinned TLC module changed`,
  );
  return {
    libraryPath: resolve(directory),
    modules: [{ ...apalacheModule, path }],
  };
};
