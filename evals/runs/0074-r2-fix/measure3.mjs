import { pathToFileURL } from "node:url";
const [dist, n, form] = process.argv.slice(2);
const count = Number(n);
const terms = Array.from({ length: count }, () => "(x' = 0)");
const body = form === "bullet" ? "\n" + terms.map((t) => `  \\/ ${t}`).join("\n") : terms.join(" \\/ ");
const source = `---- MODULE Sizes ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == ${body}\nInv == x >= 0\n====\n`;
const input = { source, config: "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n", irVersion: 2 };
const { compileTlaSafety } = await import(pathToFileURL(`${dist}/packages/idealis/src/tla-safety.js`).href);
const t = performance.now();
let out;
try { out = JSON.stringify(compileTlaSafety(input)).slice(0, 100); } catch (e) { out = `${e.name}: ${e.message}`.slice(0, 100); }
console.log(JSON.stringify({ n: count, form, ms: Math.round(performance.now() - t), out }));
