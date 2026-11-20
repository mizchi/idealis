import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, loadavg } from "node:os";
import { join, resolve } from "node:path";

const [mode, output, beforeRoot, afterRoot, quintDir, cappedPath] = process.argv.slice(2);
assert.ok((mode === "time" || mode === "capped") && output && beforeRoot && afterRoot && quintDir);
assert.ok(mode === "time" || cappedPath);
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const bounds = { maxDepth: 10000000, maxStates: 10000000, maxWork: 4000000000 };
const config = "INIT q_init\nNEXT q_step\nINVARIANT q_inv\nCHECK_DEADLOCK FALSE\n";
const sources = {
  safety: await readFile(join(quintDir, "NondetTags-safety.tla"), "utf8"),
  broken: await readFile(join(quintDir, "NondetTags-broken.tla"), "utf8"),
};
const roots = { before: resolve(beforeRoot), after: resolve(afterRoot) };
const binaries = {};
for (const [side, root] of Object.entries(roots)) {
  const bytes = await readFile(join(root, "idealis-core"));
  const manifest = JSON.parse(await readFile(join(root, "manifest.json"), "utf8"));
  assert.equal(sha(bytes), manifest.files.find((file) => file.path === "idealis-core").sha256);
  binaries[side] = sha(bytes);
}
const workerCounts = [1, 2, 4, 8];
await mkdir(output);
const save = async (name, value) =>
  writeFile(join(output, name), `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" });

const once = (path, lines) =>
  new Promise((resolveRun, reject) => {
    const child = spawn(path, [], { stdio: ["pipe", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => (stdout += String(chunk)));
    child.stderr.on("data", (chunk) => (stderr += String(chunk)));
    child.once("error", reject);
    child.once("close", (code) => resolveRun({ code, stdout, stderr }));
    child.stdin.end(lines);
  });

const programs = {};
for (const side of ["before", "after"])
  for (const property of ["safety", "broken"]) {
    const response = await once(
      join(roots[side], "idealis-core"),
      `${JSON.stringify({ version: 1, request: { kind: "compile-tla", source: sources[property], config, irVersion: 2 } })}\n`,
    );
    assert.equal(response.code, 0, response.stderr);
    const message = JSON.parse(response.stdout.trim().split("\n")[0]);
    assert.equal(message.kind, "compiled", response.stdout.slice(0, 300));
    assert.equal(message.program.version, 2);
    programs[`${side}-${property}`] = message.program;
  }
await save("manifest.json", {
  driver: "nondet-native-rss.mjs",
  mode,
  driverSha256: sha(await readFile(new URL(import.meta.url))),
  config,
  bounds,
  irVersion: 2,
  workers: workerCounts,
  warmup: 1,
  samples: 3,
  order: "iteration 0 is the warmup; even iterations run before then after, odd iterations run after then before",
  request:
    mode === "time"
      ? "one fresh /usr/bin/time -l idealis-core process per sample; stdin holds one check-safety-parallel request with the program compiled beforehand by the same side's binary (irVersion 2); maxRssBytes is the kernel maximum resident set size of the time wrapper child"
      : "one fresh process per sample under capped.py (RSS cap and wall cap, peak_rss_kb polled every 0.2 s, elapsed_s quantised by that poll); stdin holds one check-safety-parallel request with the precompiled irVersion 2 program",
  cappedPath: cappedPath ?? null,
  cappedSha256: cappedPath ? sha(await readFile(cappedPath)) : null,
  sourceSha256: { safety: sha(sources.safety), broken: sha(sources.broken) },
  programSha256: Object.fromEntries(
    Object.entries(programs).map(([key, value]) => [key, sha(JSON.stringify(value))]),
  ),
  binaries,
  roots,
  cpu: cpus()[0]?.model,
  cpus: cpus().length,
  hostLoadAtStart: loadavg(),
});

const sample = async (side, property, workers, tag) => {
  const stdinPath = join(output, `stdin-${tag}.json`);
  await writeFile(
    stdinPath,
    `${JSON.stringify({ version: 1, request: { kind: "check-safety-parallel", program: programs[`${side}-${property}`], bounds, workers } })}\n`,
    { flag: "wx" },
  );
  const exe = join(roots[side], "idealis-core");
  const loadBefore = loadavg();
  const started = performance.now();
  let run;
  let extra;
  if (mode === "time") {
    const child = spawn("/usr/bin/time", ["-l", exe], { stdio: ["pipe", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => (stdout += String(chunk)));
    child.stderr.on("data", (chunk) => (stderr += String(chunk)));
    child.stdin.end(await readFile(stdinPath));
    const code = await new Promise((resolveClose, reject) => {
      child.once("error", reject);
      child.once("close", resolveClose);
    });
    const rss = /(\d+)\s+maximum resident set size/.exec(stderr);
    assert.ok(rss, "time -l did not report maximum resident set size");
    run = { code, stdout, stderr };
    extra = { maxRssBytes: Number(rss[1]) };
  } else {
    const stdoutPath = join(output, `stdout-${tag}.json`);
    const stderrPath = join(output, `stderr-${tag}.txt`);
    const python = spawnSync(
      "python3",
      [cappedPath, String(8 * 1024 * 1024), "120", stdinPath, stdoutPath, stderrPath, exe],
      { encoding: "utf8" },
    );
    assert.equal(python.status, 0, python.stdout + python.stderr);
    const report = JSON.parse(python.stdout);
    assert.equal(report.killed, null);
    run = { code: report.exit, stdout: await readFile(stdoutPath, "utf8"), stderr: await readFile(stderrPath, "utf8") };
    extra = { capped: report, peakRssKb: report.peak_rss_kb, peakRssBytes: report.peak_rss_kb * 1024 };
  }
  const wallMs = performance.now() - started;
  assert.equal(run.code, 0, run.stderr);
  const message = JSON.parse(run.stdout.trim().split("\n")[0]);
  assert.equal(message.kind, "safety", run.stdout.slice(0, 300));
  return { side, property, workers, wallMs, ...extra, result: message.result, loadBefore, loadAfter: loadavg() };
};

const samples = [];
const controls = [];
try {
  for (const workers of workerCounts) {
    for (let iteration = 0; iteration < 4; iteration++) {
      const order = iteration % 2 === 0 ? ["before", "after"] : ["after", "before"];
      for (const side of order) {
        const tag = `safety-w${workers}-i${iteration}-${side}`;
        const record = { iteration, warmup: iteration === 0, ...(await sample(side, "safety", workers, tag)) };
        samples.push(record);
        await save(`${tag}.json`, record);
        console.log(
          `w${workers} i${iteration} ${side}: wall ${(record.wallMs / 1000).toFixed(3)}s rss ${((record.maxRssBytes ?? record.peakRssBytes) / 1048576).toFixed(1)}MiB states=${record.result.states} generated=${record.result.generated} work=${record.result.work} load=${record.loadBefore[0].toFixed(2)}`,
        );
      }
    }
  }
  for (const workers of workerCounts)
    for (const side of ["before", "after"]) {
      const tag = `broken-w${workers}-${side}`;
      const record = await sample(side, "broken", workers, tag);
      controls.push(record);
      await save(`${tag}.json`, record);
      console.log(`broken w${workers} ${side}: status=${record.result.status} states=${record.result.states} generated=${record.result.generated} work=${record.result.work}`);
    }
  await save("result.json", { status: "complete", samples, controls });
} catch (error) {
  await save("error.json", { status: "error", message: String(error), samples, controls });
  throw error;
}
