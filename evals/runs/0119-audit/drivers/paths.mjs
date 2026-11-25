// usage: node paths.mjs <side> <snapshotDir> <sideDist> <toolDist> <casesJson[,casesJson]> <quintDir> <out.json>
// Compiles and checks every case and the Quint products on four paths, IR v1 and v2.
import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const [side, snapshot, sideDist, toolDist, casesJson, quintDir, out] = process.argv.slice(2);
const tool = (p) => import(pathToFileURL(join(toolDist, p)).href);
const { observe, bounds } = await tool("evals/experiments/nesting-chains/outcome.js");
const { nativeFrontend } = await tool("evals/experiments/nesting-chains/frontends.js");
const { startNative } = await tool("evals/experiments/usable/native.js");
const { createPortableWasm } = await tool("packages/idealis/src/portable-wasm.js");
const { PortableCoreError } = await tool("packages/idealis/src/portable-value.js");
const isRecord = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
const normalize = (value) => {
  if (Array.isArray(value)) return value.map(normalize);
  if (!isRecord(value)) return value;
  return Object.fromEntries(Object.entries(value)
    .filter(([key, entry]) => !(key === "subscript" && entry === null) && !(["fairness", "initialProperties", "actionProperties", "properties"].includes(key) && Array.isArray(entry) && !entry.length))
    .map(([key, entry]) => [key, normalize(entry)]));
};
const sideTs = await import(pathToFileURL(join(sideDist, "packages/idealis/src/tla-safety.js")).href);

const sha = (v) => createHash("sha256").update(typeof v === "string" ? v : JSON.stringify(v)).digest("hex");
const core = await createPortableWasm(await readFile(join(snapshot, "idealis_core.wasm")));
let native = startNative(join(snapshot, "idealis-core"));
const restart = () => { native = startNative(join(snapshot, "idealis-core")); };

const guarded = (run) => {
  try { return run(); } catch (error) {
    if (error instanceof PortableCoreError) return { kind: "error", message: error.message };
    throw error;
  }
};
const wasmCompile = async (input) => {
  const reply = core.request({ version: 1, request: { kind: "compile-tla", ...input } });
  if (reply.kind !== "compiled") throw new Error(String(reply.message));
  return reply.program;
};
const wasmCheck = async (program) => guarded(() => core.request({ version: 1, request: { kind: "check-safety", program, bounds } }));
const tsCompile = async (input) => sideTs.compileTlaSafety(input);
const nat = () => nativeFrontend((c) => native.request(c));

const seen = new Set();
const cases = (await Promise.all(casesJson.split(",").map(async (path) => JSON.parse(await readFile(path, "utf8"))))).flat().filter((c) => !seen.has(c.name) && seen.add(c.name)).map((c) => ({ name: c.name, source: c.source, config: c.config }));
for (const m of ["LongConjunction", "LongIfChain"])
  for (const p of ["safety", "broken"])
    cases.push({ name: `quint:${m}-${p}`, source: await readFile(join(quintDir, `${m}.tla`), "utf8"), config: await readFile(join(quintDir, `${m}-${p}.cfg`), "utf8") });

const result = { side, snapshot, count: cases.length, cases: {} };
for (const c of cases) {
  const row = {};
  for (const irVersion of [1, 2]) {
    const input = { source: c.source, config: c.config, irVersion };
    const paths = {
      "ts-wasm": [tsCompile, wasmCheck],
      "ts-native": [tsCompile, (p) => nat().check(p)],
      "rust-wasm": [wasmCompile, wasmCheck],
      "rust-native": [(i) => nat().compile(i), (p) => nat().check(p)],
    };
    const cell = {};
    for (const [name, [compile, check]] of Object.entries(paths)) {
      try {
        const o = await observe(compile, check, input);
        cell[name] = { outcome: o.outcome, ir: o.program ? sha(o.program) : null, irNormalized: o.program ? sha(normalize(o.program)) : null };
      } catch (error) {
        cell[name] = { outcome: { stage: "exception", message: String(error?.message ?? error).slice(0, 300) }, ir: null, irNormalized: null };
        if (String(error?.message).includes("Native process")) restart();
      }
    }
    row[`v${irVersion}`] = cell;
  }
  result.cases[c.name] = row;
}
try { await native.close(); } catch (e) { result.nativeCloseError = String(e); }
await writeFile(out, JSON.stringify(result, null, 1) + "\n", { flag: "wx" });
console.log(`wrote ${out}: ${cases.length} cases`);
