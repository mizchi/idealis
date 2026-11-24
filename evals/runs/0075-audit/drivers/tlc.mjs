// usage: node tlc.mjs <outDir (must not exist)> <casesJson> <quintDir> [filter-regex]
// One TLC process at a time, per-case directory with its own scratch (java.io.tmpdir) and metadir.
import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { delimiter, join, resolve } from "node:path";

const [out, casesJson, quintDir, filter] = process.argv.slice(2); // quintDir "-" skips the Quint products
const E = "<scratch>";
const java = "<mise>/installs/java/temurin-21.0.11+10.0.LTS/bin/java";
const tlc = `${E}/tools/tla2tools.jar`, apalache = `${E}/tools/apalache.jar`;
await mkdir(out);
const cases = JSON.parse(await readFile(casesJson, "utf8")).map((c) => ({ name: c.name, source: c.source, config: c.config, quint: false }));
for (const m of quintDir === "-" ? [] : ["LongConjunction", "LongIfChain"])
  for (const p of ["safety", "broken"])
    cases.push({ name: `quint:${m}-${p}`, source: await readFile(join(quintDir, `${m}.tla`), "utf8"), config: await readFile(join(quintDir, `${m}-${p}.cfg`), "utf8"), quint: true });
const re = filter ? new RegExp(filter) : null;
const results = {};
for (const c of cases) {
  if (re && !re.test(c.name)) continue;
  const dir = join(out, c.name.replace(/[^A-Za-z0-9_.-]/g, "_"));
  await mkdir(dir); await mkdir(join(dir, "scratch")); await mkdir(join(dir, "states"));
  const module = /MODULE\s+(\w+)/.exec(c.source)[1];
  await writeFile(join(dir, `${module}.tla`), c.source, { flag: "wx" });
  await writeFile(join(dir, `${module}.cfg`), c.config, { flag: "wx" });
  const args = ["-Xmx256m", "-Xss8m", "-XX:+UseParallelGC", `-Djava.io.tmpdir=${join(dir, "scratch")}`, "-cp", (c.quint ? [tlc, apalache] : [tlc]).join(delimiter), "tlc2.TLC", "-workers", "1", "-nowarning", "-metadir", join(dir, "states"), "-config", `${module}.cfg`, `${module}.tla`];
  const t0 = Date.now();
  const r = spawnSync(java, args, { cwd: dir, encoding: "utf8", maxBuffer: 64 * 1024 * 1024, timeout: 180000 });
  const log = (r.stdout ?? "") + (r.stderr ?? "");
  await writeFile(join(dir, "tlc.log"), log, { flag: "wx" });
  await writeFile(join(dir, "command.json"), JSON.stringify([java, ...args], null, 1), { flag: "wx" });
  const counts = /(\d+) states generated, (\d+) distinct states found/.exec(log);
  const stateLines = [...log.matchAll(/^State (\d+):/gm)].length;
  const errors = [...log.matchAll(/^Error: (.*)$/gm)].map((m) => m[1]);
  const completed = /Model checking completed\. No error has been found/.test(log);
  results[c.name] = { exit: r.status, signal: r.signal, timedOut: r.error?.code === "ETIMEDOUT", ms: Date.now() - t0, completed, states: counts ? +counts[2] : null, generated: counts ? +counts[1] : null, trace: stateLines, errors: errors.slice(0, 3), dir };
}
await writeFile(join(out, "results.json"), JSON.stringify(results, null, 1) + "\n", { flag: "wx" });
console.log(`TLC runs: ${Object.keys(results).length}`);
