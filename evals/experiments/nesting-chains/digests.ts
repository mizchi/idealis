import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { corpora, fingerprint, sha256, type Surface } from "./corpora.js";
import { openBuild } from "./frontends.js";

const [binary, wasm, distribution, output] = process.argv.slice(2);
assert.ok(
  binary && wasm && distribution && output,
  "binary wasm distribution output",
);
const build = await openBuild(binary, wasm, distribution);
const digests: Record<string, Record<Surface, string>> = {};
for (const input of await corpora())
  digests[input.name] = {
    ts: sha256(await fingerprint(build.ts.compile, input)),
    wasm: sha256(await fingerprint(build.wasm.compile, input)),
    native: sha256(await fingerprint(build.rust.compile, input)),
  };
await build.close();
await writeFile(output, `${JSON.stringify(digests, null, 2)}\n`, {
  flag: "wx",
});
