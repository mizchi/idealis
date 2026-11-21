import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, loadavg } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";

const [output, beforeRoot, afterRoot, quintDir] = process.argv.slice(2);
assert.ok(output && beforeRoot && afterRoot && quintDir);
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
  const entry = manifest.files.find((file) => file.path === "idealis-core");
  assert.equal(sha(bytes), entry.sha256);
  binaries[side] = sha(bytes);
}
await mkdir(output);
const save = async (name, value) =>
  writeFile(join(output, name), `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" });
await save("manifest.json", {
  driver: "nondet-native.mjs",
  driverSha256: sha(await readFile(new URL(import.meta.url))),
  config,
  bounds,
  irVersion: 2,
  workers: [1, 2, 4],
  warmup: 1,
  samples: 3,
  order: "iteration 0 is the warmup; even iterations run before then after, odd iterations run after then before",
  request: "compile-tla (irVersion 2) then check-safety-parallel with workers, one fresh idealis-core process per sample",
  sourceSha256: { safety: sha(sources.safety), broken: sha(sources.broken) },
  binaries,
  roots,
  cpu: cpus()[0]?.model,
  cpus: cpus().length,
});

const session = (path) => {
  const child = spawn(path, [], { stdio: ["pipe", "pipe", "pipe"] });
  const pending = [];
  let stderr = "";
  const closed = new Promise((resolveClose) =>
    child.once("close", (code) => {
      for (const request of pending.splice(0))
        request.reject(new Error(`closed ${code} ${stderr}`));
      resolveClose(code);
    }),
  );
  child.stderr.on("data", (chunk) => {
    stderr += String(chunk);
  });
  const lines = createInterface({ input: child.stdout });
  lines.on("line", (line) => {
    const request = pending.shift();
    if (request) request.resolve(JSON.parse(line));
  });
  return {
    request: (command) =>
      new Promise((resolveRequest, reject) => {
        pending.push({ resolve: resolveRequest, reject });
        child.stdin.write(`${JSON.stringify(command)}\n`);
      }),
    close: async () => {
      child.stdin.end();
      const code = await closed;
      lines.close();
      assert.equal(code, 0, stderr);
    },
  };
};

const sample = async (side, property, workers) => {
  const loadBefore = loadavg();
  const started = performance.now();
  const core = session(join(roots[side], "idealis-core"));
  try {
    const compileStart = performance.now();
    const compiled = await core.request({
      version: 1,
      request: {
        kind: "compile-tla",
        source: sources[property],
        config,
        irVersion: 2,
      },
    });
    const compileMs = performance.now() - compileStart;
    assert.equal(compiled.kind, "compiled", JSON.stringify(compiled).slice(0, 500));
    const checkStart = performance.now();
    const response = await core.request({
      version: 1,
      request: {
        kind: "check-safety-parallel",
        program: compiled.program,
        bounds,
        workers,
      },
    });
    const checkMs = performance.now() - checkStart;
    assert.equal(response.kind, "safety", JSON.stringify(response).slice(0, 500));
    return {
      side,
      property,
      workers,
      compileMs,
      checkMs,
      wallMs: performance.now() - started,
      programVersion: compiled.program.version,
      programSha256: sha(JSON.stringify(compiled.program)),
      result: response.result,
      loadBefore,
      loadAfter: loadavg(),
    };
  } finally {
    await core.close();
  }
};

const samples = [];
const controls = [];
try {
  for (const workers of [1, 2, 4]) {
    for (let iteration = 0; iteration < 4; iteration++) {
      const order = iteration % 2 === 0 ? ["before", "after"] : ["after", "before"];
      for (const side of order) {
        const record = { iteration, warmup: iteration === 0, ...(await sample(side, "safety", workers)) };
        samples.push(record);
        await save(`safety-w${workers}-i${iteration}-${side}.json`, record);
        console.log(
          `w${workers} i${iteration} ${side}: check ${(record.checkMs / 1000).toFixed(3)}s wall ${(record.wallMs / 1000).toFixed(3)}s states=${record.result.states} generated=${record.result.generated} work=${record.result.work} load=${record.loadBefore[0].toFixed(2)}`,
        );
      }
    }
  }
  for (const workers of [1, 2, 4]) {
    for (const side of ["before", "after"]) {
      const record = await sample(side, "broken", workers);
      controls.push(record);
      await save(`broken-w${workers}-${side}.json`, record);
      console.log(`broken w${workers} ${side}: status=${record.result.status} states=${record.result.states} generated=${record.result.generated} work=${record.result.work}`);
    }
  }
  await save("result.json", { status: "complete", samples, controls });
} catch (error) {
  await save("error.json", { status: "error", message: String(error), samples, controls });
  throw error;
}
