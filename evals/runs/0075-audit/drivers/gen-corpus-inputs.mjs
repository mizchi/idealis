// usage (cwd = branch worktree): node gen-corpus-inputs.mjs <toolDist> <out.json>
// Generates the input list once so that both sides compile identical inputs.
import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { pathToFileURL } from "node:url";
import { join } from "node:path";
const [toolDist, out] = process.argv.slice(2);
const tool = (p) => import(pathToFileURL(join(toolDist, p)).href);
const { corpora } = await tool("evals/experiments/nesting-chains/corpora.js");
const { createAnalysisCorpus } = await tool("tests/tlc-differential/analysis-parity-corpus.js");
const dir = "fixtures/tlc-operator-sharing";
const inventory = JSON.parse(await readFile(`${dir}/analysis-inventory.json`, "utf8"));
const boundary = JSON.parse(await readFile(`${dir}/analysis-boundary-manifest.json`, "utf8"));
const inputs = [];
for (const c of await corpora()) inputs.push({ name: c.name, source: c.source, config: c.config, group: "corpora" });
for (const c of createAnalysisCorpus(inventory.sharedApplyArityEntries).cases)
  inputs.push({ name: `analysis-corpus:${c.id}`, source: c.input.source, config: c.input.config, group: "analysis-corpus" });
for (const c of boundary.cases) inputs.push({ name: `analysis-boundary:${c.id}`, source: c.source, config: c.config, group: "analysis-boundary" });
for (const c of inventory.syntaxContexts)
  inputs.push({ name: `analysis-inventory-syntax:${c.id}`, source: c.source, config: "INIT Init\nNEXT Next\nINVARIANT Probe\nCHECK_DEADLOCK FALSE", group: "analysis-inventory" });
const seen = new Set(); const unique = [];
for (const i of inputs) { const k = JSON.stringify([i.source, i.config]); if (seen.has(k)) continue; seen.add(k); unique.push(i); }
const body = JSON.stringify(unique);
await writeFile(out, body, { flag: "wx" });
const counts = {}; for (const i of unique) counts[i.group] = (counts[i.group] ?? 0) + 1;
console.log(JSON.stringify({ total: inputs.length, unique: unique.length, counts, sha256: createHash("sha256").update(body).digest("hex") }));
