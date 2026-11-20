// usage: node ir-corpora.mjs <side> <snapshotDir> <sideDist> <toolDist> <inputs.json> <out.json>
import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { pathToFileURL } from "node:url";
import { join } from "node:path";
const [side, snapshot, sideDist, toolDist, inputsJson, out] = process.argv.slice(2);
const tool = (p) => import(pathToFileURL(join(toolDist, p)).href);
const { nativeFrontend } = await tool("evals/experiments/nesting-chains/frontends.js");
const { startNative } = await tool("evals/experiments/usable/native.js");
const { createPortableWasm } = await tool("packages/idealis/src/portable-wasm.js");
const sideTs = await import(pathToFileURL(join(sideDist, "packages/idealis/src/tla-safety.js")).href);
const isRecord = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
const normalize = (value) => {
  if (Array.isArray(value)) return value.map(normalize);
  if (!isRecord(value)) return value;
  return Object.fromEntries(Object.entries(value)
    .filter(([key, entry]) => !(key === "subscript" && entry === null) && !(["fairness", "initialProperties", "actionProperties", "properties"].includes(key) && Array.isArray(entry) && !entry.length))
    .map(([key, entry]) => [key, normalize(entry)]));
};
const sha = (v) => createHash("sha256").update(typeof v === "string" ? v : JSON.stringify(v)).digest("hex");
const core = await createPortableWasm(await readFile(join(snapshot, "idealis_core.wasm")));
let native = startNative(join(snapshot, "idealis-core"));
const nat = nativeFrontend((c) => native.request(c));
const surfaces = {
  ts: async (i) => sideTs.compileTlaSafety(i),
  wasm: async (i) => { const r = core.request({ version: 1, request: { kind: "compile-tla", ...i } }); if (r.kind !== "compiled") throw new Error(String(r.message)); return r.program; },
  native: (i) => nat.compile(i),
};
const inputs = JSON.parse(await readFile(inputsJson, "utf8"));
const result = { side, count: inputs.length, inputsSha256: sha(await readFile(inputsJson, "utf8")), cells: {} };
for (const input of inputs) {
  const row = {};
  for (const irVersion of [1, 2]) {
    const cell = {};
    for (const [name, compile] of Object.entries(surfaces)) {
      try {
        const program = await compile({ source: input.source, config: input.config, irVersion });
        cell[name] = `p:${sha(program)}:${sha(normalize(program))}`;
      } catch (error) {
        cell[name] = `e:${String(error?.message ?? error).slice(0, 400)}`;
      }
    }
    row[`v${irVersion}`] = cell;
  }
  result.cells[input.name] = row;
}
await native.close();
await writeFile(out, JSON.stringify(result) + "\n", { flag: "wx" });
console.log(`wrote ${out}: ${inputs.length} inputs`);
