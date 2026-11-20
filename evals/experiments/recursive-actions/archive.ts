import assert from "node:assert/strict";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { zipSync } from "fflate";
import { digest } from "../usable/provenance.js";

const [directory, label] = process.argv.slice(2);
assert.ok(directory && label && /^[a-z-]+$/.test(label));
await mkdir(directory, { recursive: true });
const files: Record<string, Uint8Array> = {};
const collect = async (path: string): Promise<void> => {
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = join(path, entry.name);
    if (entry.isDirectory()) {
      if (!["source", "target"].includes(entry.name)) await collect(child);
    } else if (!["benchmark", "recursive-profile"].includes(entry.name)) {
      files[child] = await readFile(child);
    }
  }
};
for (const entry of await readdir(".artifacts", { withFileTypes: true })) {
  if (!entry.name.startsWith("0071")) continue;
  const path = join(".artifacts", entry.name);
  if (entry.isDirectory()) await collect(path);
  else files[path] = await readFile(path);
}
const bytes = zipSync(files, { level: 9 });
const archive = `${label}.zip`;
await writeFile(join(directory, archive), bytes, { flag: "wx" });
await writeFile(
  join(directory, `${label}.json`),
  `${JSON.stringify(
    {
      plan: "0071-recursive-actions",
      archive,
      sha256: digest(bytes),
      exclusions:
        "Generated build source trees/target caches and executable copies. Immutable source.zip, supplements, raw logs, binary hashes and rebuild results are included.",
      files: Object.entries(files).map(([path, contents]) => ({
        path,
        sha256: digest(contents),
      })),
    },
    null,
    2,
  )}\n`,
  { flag: "wx" },
);
