import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";
const [binary, wasmPath, dist, n, surface] = process.argv.slice(2);
const count = Number(n);
const terms = Array.from({ length: count }, () => "(x' = 0)");
const source = `---- MODULE Sizes ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == \n${terms.map((t) => `  \\/ ${t}`).join("\n")}\nInv == x >= 0\n====\n`;
const input = { source, config: "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n", irVersion: 2 };
const t = performance.now();
let out;
if (surface === "native") {
  const r = spawnSync(binary, [], { input: JSON.stringify({ version: 1, request: { kind: "compile-tla", ...input } }) + "\n", maxBuffer: 1 << 28, timeout: 120000 });
  out = r.stdout ? r.stdout.toString().slice(0, 160) : `signal ${r.signal} ${r.stderr}`;
} else if (surface === "wasm") {
  const { createPortableWasm } = await import(pathToFileURL(`${dist}/packages/idealis/src/portable-wasm.js`).href);
  const core = await createPortableWasm(readFileSync(wasmPath));
  try { out = JSON.stringify(core.request({ version: 1, request: { kind: "compile-tla", ...input } })).slice(0, 160); } catch (e) { out = String(e.message).slice(0, 160); }
} else {
  const { compileTlaSafety } = await import(pathToFileURL(`${dist}/packages/idealis/src/tla-safety.js`).href);
  try { out = JSON.stringify(compileTlaSafety(input)).slice(0, 160); } catch (e) { out = `${e.name}: ${e.message}`.slice(0, 160); }
}
console.log(JSON.stringify({ surface, n: count, ms: Math.round(performance.now() - t), out }));
