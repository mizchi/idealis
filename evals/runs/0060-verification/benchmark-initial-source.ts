import assert from "node:assert/strict";
import { isRecord } from "../../src/trace.js";
import { decodeSafetyResult } from "../../src/safety.js";
import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { loadavg } from "node:os";
import { join, resolve } from "node:path";
import { prepare, runTool } from "../single-model/runner.js";
import type { BenchmarkCase } from "../single-model/cases.js";
import { environment } from "../checker-benchmark/provenance.js";
import { digest } from "../usable/provenance.js";
import { counter, plain } from "./cases.js";
import { symmetryFingerprint } from "./provenance.js";

const [directory, java, jar, archive, binary] = process.argv.slice(2);
assert.ok(directory && java && jar && archive && binary);
await mkdir(directory);
const source = await symmetryFingerprint();
const nativeSha256 = digest(await readFile(binary));
const input = counter(7, 5);
const routes = [
  { name: "native-plain-1", backend: "native", symmetry: false, workers: 1 },
  { name: "native-symmetry-1", backend: "native", symmetry: true, workers: 1 },
  { name: "native-plain-8", backend: "native", symmetry: false, workers: 8 },
  {
    name: "native-symmetry-8-request",
    backend: "native",
    symmetry: true,
    workers: 8,
  },
  { name: "tlc-plain-1", backend: "tlc-1", symmetry: false, workers: 1 },
  { name: "tlc-symmetry-1", backend: "tlc-1", symmetry: true, workers: 1 },
  { name: "tlc-plain-8", backend: "tlc-8", symmetry: false, workers: 8 },
  { name: "tlc-symmetry-8", backend: "tlc-8", symmetry: true, workers: 8 },
] as const;
const files: string[] = [];
const save = async (path: string, value: unknown) => {
  await writeFile(
    join(directory, path),
    `${JSON.stringify(value, null, 2)}\n`,
    { flag: "wx" },
  );
  files.push(path);
};
await save("manifest.json", {
  plan: "0060-restricted-symmetry",
  ...source,
  nativeSha256,
  environment: await environment(java, jar, archive),
  routes,
  input,
  warmup: 1,
  measured: 3,
  scope:
    "native wall includes Node host, Rust frontend, startup, IPC and check; symmetry worker requests fall back to serial",
});
const samples = [];
try {
  for (let iteration = 0; iteration < 4; iteration++) {
    const rotated = routes.map(
      (_, index) => routes[(index + iteration) % routes.length],
    );
    for (const route of iteration % 2 === 0 ? rotated : rotated.reverse()) {
      assert.ok(route);
      const selected = route.symmetry ? input : plain(input);
      const states = route.symmetry ? 792 : 279936;
      const generated = route.symmetry ? 5545 : 1959553;
      const testCase: BenchmarkCase = {
        ...selected,
        states,
        generated,
        initialStates: 1,
        layerWidth: 1,
        faultPredicate: "unused",
      };
      const name = `${iteration}-${route.name}`;
      const path = join(directory, name);
      await prepare(path, testCase);
      const hostLoadBefore = loadavg();
      let actual;
      if (route.backend === "native") {
        const args = [
          "--max-old-space-size=8192",
          resolve("dist/evals/symmetry/worker.js"),
          resolve(path, "input.json"),
          String(route.workers),
          resolve(binary),
        ];
        const started = performance.now();
        const child = spawn(process.execPath, args, {
          stdio: ["ignore", "pipe", "pipe"],
        });
        let stdout = "";
        let stderr = "";
        child.stdout.on("data", (chunk) => {
          stdout += String(chunk);
        });
        child.stderr.on("data", (chunk) => {
          stderr += String(chunk);
        });
        const exit = await new Promise<number | null>((resolveExit, reject) => {
          child.once("error", reject);
          child.once("close", resolveExit);
        });
        const wallMs = performance.now() - started;
        assert.equal(exit, 0, stderr);
        const output: unknown = JSON.parse(stdout);
        assert.ok(isRecord(output));
        const result = decodeSafetyResult(output.result);
        assert.equal(result.status, "complete");
        assert.equal(result.states, states);
        assert.equal(result.generated, generated);
        actual = {
          command: process.execPath,
          args,
          exit,
          stdout,
          stderr,
          output,
          wallMs,
        };
      } else {
        actual = await runTool(route.backend, path, testCase, java, jar);
        assert.ok(isRecord(actual.output));
        assert.equal(actual.output.status, "complete");
        assert.equal(actual.output.states, states);
        assert.equal(actual.output.generated, generated);
      }
      await save(`${name}.json`, actual);
      samples.push({
        iteration,
        warmup: iteration === 0,
        route: route.name,
        wallMs: actual.wallMs,
        hostLoadBefore,
        hostLoadAfter: loadavg(),
      });
      console.log(name, `${(actual.wallMs / 1000).toFixed(3)}s`);
    }
  }
  assert.equal(
    (await symmetryFingerprint()).sourceVersion,
    source.sourceVersion,
  );
  assert.equal(digest(await readFile(binary)), nativeSha256);
  await save("result.json", {
    status: "complete",
    samples,
    files: await Promise.all(
      files.map(async (path) => ({
        path,
        sha256: digest(await readFile(join(directory, path))),
      })),
    ),
  });
} catch (error) {
  await save("error.json", { status: "error", error: String(error), samples });
  throw error;
}
