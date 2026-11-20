import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import { gzipSync } from "node:zlib";
import type {
  PortableCommand,
  SafetyProgram,
} from "../../../packages/idealis/src/contracts.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyProgram } from "../../../packages/idealis/src/safety-program.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { sharing } from "../p0-checker/inputs.js";
import { config, names } from "../quint-generated/cases.js";
import { bounds, cases } from "../single-model/cases.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";

const [before, after, directory] = process.argv.slice(2);
const varied = process.env.IDEALIS_PREPARED_BUDGETS === "1";
assert.ok(before && after && directory);
await mkdir(directory);
const plan = await readFile("evals/plans/0064-prepared-safety.md");
await writeFile(join(directory, "plan.md"), plan, { flag: "wx" });
const save = (path: string, value: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const packed = async (path: string, value: unknown) => {
  const bytes = gzipSync(JSON.stringify(value));
  await writeFile(join(directory, path), bytes, { flag: "wx" });
  return { path, sha256: digest(bytes) };
};
const inputs = [
  ...(await Promise.all(
    names.map(async (name) => ({
      name,
      source: await readFile(
        `fixtures/quint-generated/generated/${name}.tla`,
        "utf8",
      ),
      config: config("safety"),
    })),
  )),
  {
    name: "TCommit-8",
    source: await readFile("fixtures/tcommit/TCommit.tla", "utf8"),
    config: await readFile("fixtures/tcommit/TCommit.cfg", "utf8"),
  },
  ...cases("small"),
  ...[10, 60].map((depth) => ({ name: `sharing-${depth}`, ...sharing(depth) })),
];
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
  {
    name: "before-native",
    mode: "one-shot",
    native: true,
    root: before,
    core: nativeBefore,
  },
  {
    name: "after-native",
    mode: "one-shot",
    native: true,
    root: after,
    core: nativeAfter,
  },
  {
    name: "reuse-native",
    mode: "reuse",
    native: true,
    root: after,
    core: nativeAfter,
  },
  {
    name: "batch-native",
    mode: "batch",
    native: true,
    root: after,
    core: nativeAfter,
  },
  {
    name: "before-wasm",
    mode: "one-shot",
    native: false,
    root: before,
    core: wasmBefore,
  },
  {
    name: "after-wasm",
    mode: "one-shot",
    native: false,
    root: after,
    core: wasmAfter,
  },
  {
    name: "reuse-wasm",
    mode: "reuse",
    native: false,
    root: after,
    core: wasmAfter,
  },
  {
    name: "batch-wasm",
    mode: "batch",
    native: false,
    root: after,
    core: wasmAfter,
  },
] as const;
await save("manifest.json", {
  plan: "0064-prepared-safety",
  planSha256: digest(plan),
  snapshots,
  inputs,
  bounds,
  node: process.version,
  routes: routes.map(({ core: _core, ...route }) => route),
  warmup: 1,
  samples: 3,
  scope:
    "Reused native JSONL and Node Wasm. prepare/check/release API segments exclude assertion and disk costs. Cold includes parent JSON encoding and time wrapper, not Node startup.",
  driverSha256: digest(
    await readFile("evals/experiments/prepared-safety/measure.ts"),
  ),
  variedBudgets: varied,
});
const records = [];
const cold = [];
const compiled = [];
const unsupported = [];
const resultRows = (response: unknown): unknown[] => {
  assert.ok(isRecord(response));
  if (response.kind === "safety") return [response];
  assert.equal(response.kind, "safety-batch");
  assert.ok(Array.isArray(response.results));
  return response.results;
};
try {
  for (const input of inputs)
    for (const irVersion of [1, 2] as const) {
      const request: PortableCommand = {
        version: 1,
        request: {
          kind: "compile-tla",
          source: input.source,
          config: input.config,
          irVersion,
        },
      };
      const old = await nativeBefore.request(request);
      const current = await nativeAfter.request(request);
      assert.deepEqual(current, old);
      assert.ok(isRecord(old));
      if (old.kind === "error") {
        assert.ok(
          input.name === "sharing-60" && irVersion === 1,
          JSON.stringify(old),
        );
        unsupported.push({ input: input.name, irVersion, response: old });
        continue;
      }
      assert.equal(old.kind, "compiled");
      const program: SafetyProgram = decodeSafetyProgram(old.program);
      assert.equal(program.version, irVersion);
      compiled.push({
        input: input.name,
        irVersion,
        program: await packed(
          `${input.name}-v${irVersion}-program.json.gz`,
          program,
        ),
      });
      const check: PortableCommand = {
        version: 1,
        request: { kind: "check-safety", program, bounds },
      };
      const expected = await nativeBefore.request(check);
      assert.ok(
        isRecord(expected) &&
          expected.kind === "safety" &&
          isRecord(expected.result),
      );
      assert.equal(expected.result.status, "complete");
      if (input.name === "TCommit-8") {
        assert.equal(expected.result.states, 6816);
        assert.equal(expected.result.generated, 53513);
      }
      for (const repetitions of input.name === "TCommit-8"
        ? [1, 10]
        : [1, 10, 100]) {
        const jobs = Array.from({ length: repetitions }, (_, index) => ({
          bounds: varied
            ? {
                ...bounds,
                maxWork: 2 ** (index % 20),
                maxDepth: [0, 1, 2, 4, 16][index % 5] ?? 0,
              }
            : bounds,
        }));
        const expectedResults: readonly unknown[] = varied
          ? await Promise.all(
              jobs.map((job) =>
                nativeBefore.request({
                  version: 1,
                  request: {
                    kind: "check-safety",
                    program,
                    bounds: job.bounds,
                  },
                }),
              ),
            )
          : jobs.map(() => expected);
        await save(`${input.name}-v${irVersion}-n${repetitions}-jobs.json`, {
          jobs,
          expected: expectedResults,
        });
        for (let iteration = 0; iteration < 4; iteration++) {
          const order = routes.map(
            (_, index) => routes[(index + iteration) % routes.length],
          );
          for (const route of iteration % 2 ? order.reverse() : order) {
            assert.ok(route);
            const setup = performance.now();
            const prepared =
              route.mode === "one-shot"
                ? undefined
                : await route.core.request({
                    version: 1,
                    request: { kind: "prepare-safety", program },
                  });
            let handle = 0;
            if (prepared !== undefined) {
              assert.ok(
                isRecord(prepared) &&
                  prepared.kind === "safety-prepared" &&
                  typeof prepared.handle === "number",
              );
              handle = prepared.handle;
            }
            const prepareMs = performance.now() - setup;
            const started = performance.now();
            const outputs: unknown[] = [];
            if (route.mode === "batch")
              outputs.push(
                ...resultRows(
                  await route.core.request({
                    version: 1,
                    request: {
                      kind: "check-prepared-safety-batch",
                      handle,
                      jobs,
                    },
                  }),
                ),
              );
            else
              for (const job of jobs)
                outputs.push(
                  await route.core.request(
                    route.mode === "one-shot"
                      ? {
                          version: 1,
                          request: {
                            kind: "check-safety",
                            program,
                            bounds: job.bounds,
                          },
                        }
                      : {
                          version: 1,
                          request: {
                            kind: "check-prepared-safety",
                            handle,
                            bounds: job.bounds,
                          },
                        },
                  ),
                );
            const checkMs = performance.now() - started;
            assert.equal(outputs.length, repetitions);
            for (const [index, output] of outputs.entries())
              assert.deepEqual(
                output,
                expectedResults[index],
                `${input.name}:v${irVersion}:${route.name}`,
              );
            const release = performance.now();
            if (handle)
              assert.deepEqual(
                await route.core.request({
                  version: 1,
                  request: { kind: "release-safety", handle },
                }),
                { kind: "released" },
              );
            const releaseMs = performance.now() - release;
            const name = `${input.name}-v${irVersion}-n${repetitions}-${iteration}-${route.name}`;
            records.push({
              input: input.name,
              irVersion,
              repetitions,
              iteration,
              warmup: iteration === 0,
              route: route.name,
              prepareMs,
              checkMs,
              releaseMs,
              totalMs: prepareMs + checkMs + releaseMs,
              responses: await packed(`${name}.json.gz`, outputs),
            });
          }
        }
        for (let iteration = 0; iteration < 4; iteration++) {
          const native = routes.filter((route) => route.native);
          const order = native.map(
            (_, index) => native[(index + iteration) % native.length],
          );
          for (const route of iteration % 2 ? order.reverse() : order) {
            assert.ok(route);
            const requests: PortableCommand[] =
              route.mode === "one-shot"
                ? jobs.map((job) => ({
                    version: 1,
                    request: {
                      kind: "check-safety",
                      program,
                      bounds: job.bounds,
                    },
                  }))
                : [
                    {
                      version: 1,
                      request: { kind: "prepare-safety", program },
                    },
                    ...(route.mode === "batch"
                      ? [
                          {
                            version: 1,
                            request: {
                              kind: "check-prepared-safety-batch",
                              handle: 1,
                              jobs,
                            },
                          } as const,
                        ]
                      : jobs.map(
                          (job) =>
                            ({
                              version: 1,
                              request: {
                                kind: "check-prepared-safety",
                                handle: 1,
                                bounds: job.bounds,
                              },
                            }) as const,
                        )),
                    {
                      version: 1,
                      request: { kind: "release-safety", handle: 1 },
                    },
                  ];
            const start = performance.now();
            const process = spawnSync(
              "/usr/bin/time",
              ["-l", resolve(route.root, "idealis-core")],
              {
                input: requests
                  .map((request) => `${JSON.stringify(request)}\n`)
                  .join(""),
                encoding: "utf8",
                maxBuffer: 32 * 1024 * 1024,
              },
            );
            const wallMs = performance.now() - start;
            const name = `${input.name}-v${irVersion}-n${repetitions}-${iteration}-${route.name}-cold`;
            const response = await packed(`${name}.json.gz`, {
              status: process.status,
              signal: process.signal,
              error: process.error?.message,
              stdout: process.stdout,
              stderr: process.stderr,
            });
            assert.equal(process.status, 0, process.stderr);
            const packets: unknown[] = process.stdout
              .trim()
              .split("\n")
              .map((line) => JSON.parse(line));
            assert.equal(packets.length, requests.length);
            const measured =
              route.mode === "one-shot"
                ? packets
                : packets.slice(1, -1).flatMap(resultRows);
            assert.equal(measured.length, repetitions);
            for (const [index, packet] of measured.entries())
              assert.deepEqual(packet, expectedResults[index]);
            const rss = process.stderr.match(
              /(\d+)\s+maximum resident set size/,
            );
            assert.ok(rss?.[1]);
            cold.push({
              input: input.name,
              irVersion,
              repetitions,
              iteration,
              warmup: iteration === 0,
              route: route.name,
              wallMs,
              maxRssBytes: Number(rss[1]),
              response,
            });
          }
        }
      }
    }
  for (const [root, snapshot] of [
    [before, snapshots[0]],
    [after, snapshots[1]],
  ] as const)
    for (const file of snapshot.files)
      assert.equal(digest(await readFile(join(root, file.path))), file.sha256);
  await save("result.json", {
    status: "complete",
    records,
    cold,
    compiled,
    unsupported,
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), plan: "0064-prepared-safety", kind: "reuse-benchmark", status: "complete" })}\n`,
  );
} catch (error) {
  await save("error.json", {
    status: "failed",
    error: String(error),
    records,
    cold,
    compiled,
    unsupported,
  });
  throw error;
} finally {
  await nativeBefore.close();
  await nativeAfter.close();
}
