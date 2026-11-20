import assert from "node:assert/strict";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus } from "node:os";
import { basename, join, resolve } from "node:path";
import type { PortableCommand } from "../../src/contracts.js";
import { createPortableWasm } from "../../src/portable-wasm.js";
import { decodeSafetyProgram } from "../../src/safety-program.js";
import { isRecord } from "../../src/trace.js";
import { cases } from "../single-model/cases.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";
import { bounds, joint, nodeSets, recordNodes } from "./cases.js";

interface Sample {
  readonly iteration: number;
  readonly warmup: boolean;
  readonly route: string;
  readonly compiledMs: number;
  readonly checkedMs: number;
  readonly wallMs: number;
  readonly result: unknown;
}

const [before, after, directory] = process.argv.slice(2);
assert.ok(before && after && directory);
await mkdir(directory);
const save = async (path: string, value: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const snapshots = await Promise.all(
  [before, after].map(async (root) => {
    const manifest = JSON.parse(
      await readFile(join(root, "manifest.json"), "utf8"),
    );
    for (const file of manifest.files)
      assert.equal(digest(await readFile(join(root, file.path))), file.sha256);
    return manifest;
  }),
);
const nativeBefore = startNative(resolve(before, "idealis-core"));
const nativeAfter = startNative(resolve(after, "idealis-core"));
const wasmBefore = await createPortableWasm(
  await readFile(join(before, "idealis_core.wasm")),
);
const wasmAfter = await createPortableWasm(
  await readFile(join(after, "idealis_core.wasm")),
);
const routes = [
  { name: "before-native-1", request: nativeBefore.request, workers: 1 },
  { name: "after-native-1", request: nativeAfter.request, workers: 1 },
  { name: "before-native-8", request: nativeBefore.request, workers: 8 },
  { name: "after-native-8", request: nativeAfter.request, workers: 8 },
  {
    name: "before-wasm",
    request: async (command: PortableCommand) => wasmBefore.request(command),
    workers: 1,
  },
  {
    name: "after-wasm",
    request: async (command: PortableCommand) => wasmAfter.request(command),
    workers: 1,
  },
];
const inputs = [
  recordNodes(12),
  recordNodes(5),
  nodeSets(5),
  joint(5),
  ...cases("small"),
];
await save("manifest.json", {
  plan: "0065-symmetry-allocation",
  snapshots,
  inputs,
  bounds,
  routes: routes.map(({ request, ...route }) => route),
  node: process.version,
  cpu: cpus()[0]?.model,
  warmup: 1,
  samples: 3,
  scope:
    "Reused native JSONL and Node Wasm. Source compilation plus one-shot IR check API segments; assertion and disk excluded. No process startup or TLC timing.",
  driverSha256: digest(await readFile("evals/symmetry/allocation.ts")),
});
const records = [];
try {
  for (const input of inputs) {
    const compile: PortableCommand = {
      version: 1,
      request: {
        kind: "compile-tla",
        source: input.source,
        config: input.config,
      },
    };
    const compiled = await nativeBefore.request(compile);
    assert.ok(isRecord(compiled) && compiled.kind === "compiled");
    const program = decodeSafetyProgram(compiled.program);
    assert.equal(program.version, 1);
    for (const symmetryActions of [false, true]) {
      const expected = await nativeBefore.request({
        version: 1,
        request: {
          kind: "check-safety",
          program,
          bounds,
          options: { symmetryActions },
        },
      });
      assert.ok(isRecord(expected) && expected.kind === "safety");
      assert.ok(
        isRecord(expected.result) && expected.result.status === "complete",
      );
      const samples: Sample[] = [];
      for (let iteration = 0; iteration < 4; iteration++) {
        const rotated = routes.map(
          (_, index) => routes[(index + iteration) % routes.length],
        );
        for (const route of iteration % 2 === 0 ? rotated : rotated.reverse()) {
          assert.ok(route);
          const started = performance.now();
          const response = await route.request(compile);
          const compiledMs = performance.now() - started;
          assert.deepEqual(response, compiled);
          const checking = performance.now();
          const result = await route.request({
            version: 1,
            request: {
              kind: "check-safety",
              program,
              bounds,
              options: { symmetryActions, workers: route.workers },
            },
          });
          const checkedMs = performance.now() - checking;
          assert.deepEqual(result, expected);
          samples.push({
            iteration,
            warmup: iteration === 0,
            route: route.name,
            compiledMs,
            checkedMs,
            wallMs: compiledMs + checkedMs,
            result,
          });
        }
      }
      records.push({
        input: input.name,
        symmetryActions,
        programSha256: digest(JSON.stringify(program)),
        expected,
        samples,
        summary: routes.map((route) => {
          const measured = samples
            .filter((sample) => sample.route === route.name && !sample.warmup)
            .map((sample) => sample.wallMs)
            .sort((left, right) => left - right);
          assert.equal(measured.length, 3);
          return {
            route: route.name,
            minMs: measured[0],
            medianMs: measured[1],
            maxMs: measured[2],
          };
        }),
      });
      console.log(input.name, symmetryActions ? "actions" : "symmetry");
    }
  }
  for (const [index, root] of [before, after].entries())
    for (const file of snapshots[index].files)
      assert.equal(digest(await readFile(join(root, file.path))), file.sha256);
  await save("result.json", { status: "complete", records });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "symmetry-allocation", status: "complete" })}\n`,
  );
} catch (error) {
  await save("error.json", { status: "error", error: String(error), records });
  throw error;
} finally {
  await nativeBefore.close();
  await nativeAfter.close();
}
