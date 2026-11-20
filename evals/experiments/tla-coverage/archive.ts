import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { unzipSync } from "fflate";
import { digest } from "../usable/provenance.js";

interface ArchiveIndex {
  readonly packs: readonly {
    readonly path: string;
    readonly sha256: string;
    readonly files: readonly {
      readonly path: string;
      readonly sha256: string;
    }[];
  }[];
}
export const readArchive = async (name: string) => {
  const root = "evals/runs/0070-tla-coverage";
  const index: ArchiveIndex = JSON.parse(
    await readFile(`${root}/archives.json`, "utf8"),
  );
  const entry = index.packs.find((entry) => entry.path === name);
  assert.ok(entry, `Missing archived run ${name}`);
  const bytes = await readFile(`${root}/${name}`);
  assert.equal(digest(bytes), entry.sha256, name);
  const files = unzipSync(bytes);
  assert.deepEqual(
    Object.keys(files).sort(),
    entry.files.map((file) => file.path).sort(),
  );
  for (const file of entry.files) {
    const bytes = files[file.path];
    assert.ok(bytes);
    assert.equal(digest(bytes), file.sha256, `${name}/${file.path}`);
  }
  return {
    text: (path: string) => {
      const file = files[path];
      assert.ok(file, `${name}/${path}`);
      return new TextDecoder().decode(file);
    },
  };
};
