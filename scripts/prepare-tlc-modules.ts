import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import {
  apalacheModule,
  loadTlcStandardModules,
  standardModulesDirectory,
} from "../evals/experiments/tlc-differential/standard-modules.js";

const directory = process.argv[2] ?? standardModulesDirectory;
const path = resolve(directory, apalacheModule.file);
if (!existsSync(path)) {
  const response = await fetch(apalacheModule.url);
  assert.ok(response.ok, `TLC module download failed: HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  assert.equal(
    createHash("sha256").update(bytes).digest("hex"),
    apalacheModule.sha256,
    "Downloaded TLC module does not match its pinned SHA-256",
  );
  await mkdir(directory, { recursive: true });
  await writeFile(path, bytes, { flag: "wx" });
}
const loaded = await loadTlcStandardModules(directory);
console.log(`Verified TLC standard modules: ${loaded.libraryPath}`);
