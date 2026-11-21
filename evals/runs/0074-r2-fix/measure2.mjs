import { pathToFileURL } from "node:url";
const [dist, n, k] = process.argv.slice(2).map((v, i) => (i ? Number(v) : v));
const body = Array.from({ length: k }, () => "(x' = 0)").join(" \\/ ");
const source = `---- MODULE Sizes ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nA == ${body}\nNext == ${Array.from({ length: n }, () => "A").join(" \\/ ")}\nInv == x >= 0\n====\n`;
const input = { source, config: "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n", irVersion: 2 };
const { compileTlaSafety } = await import(pathToFileURL(`${dist}/packages/idealis/src/tla-safety.js`).href);
const t = performance.now();
let out;
try { out = JSON.stringify(compileTlaSafety(input)).slice(0, 100); } catch (e) { out = `${e.name}: ${e.message}`.slice(0, 100); }
console.log(JSON.stringify({ n, k, ms: Math.round(performance.now() - t), out }));
