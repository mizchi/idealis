import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import { PortableCoreError } from "../../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { decodeSafetyProgram } from "../../../packages/idealis/src/safety-program.js";
import { decodeSafetyResult } from "../../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { environment } from "../checker-benchmark/provenance.js";
import { config, names } from "../quint-generated/cases.js";
import { bounds } from "../single-model/cases.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";
import { sharing } from "./inputs.js";

const [before, after, java, jar, archive, directory] = process.argv.slice(2);
assert.ok(before && after && java && jar && archive && directory);
await mkdir(directory);
const planName = process.env.IDEALIS_CHECKER_PLAN ?? "0062-p0-checker";
const plan = await readFile(`evals/plans/${planName}.md`);
await writeFile(join(directory, "plan.md"), plan, { flag: "wx" });
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
  ...[10, 20, 30, 60].map((depth) => ({
    name: `sharing-${depth}`,
    ...sharing(depth),
  })),
];
const oracle: unknown = JSON.parse(
  await readFile("fixtures/quint-generated/tlc-oracle.json", "utf8"),
);
assert.ok(Array.isArray(oracle));
const transaction = JSON.parse(
  await readFile("evals/runs/0062-tcommit-oracle/oracle.json", "utf8"),
);
const references = inputs.map((input) => {
  if (input.name.startsWith("sharing-"))
    return { name: input.name, status: "complete", states: 1, generated: 2 };
  if (input.name === "TCommit-8")
    return {
      name: input.name,
      status: transaction.status,
      states: transaction.states,
      generated: transaction.generated,
    };
  const entry: unknown = oracle.find(
    (value) =>
      isRecord(value) &&
      value.name === input.name &&
      value.property === "safety",
  );
  assert.ok(isRecord(entry));
  assert.equal(entry.tlaSha256, digest(input.source));
  assert.equal(entry.configSha256, digest(input.config));
  return {
    name: input.name,
    status: entry.status,
    states: entry.states,
    generated: entry.generated,
  };
});
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
const specs = [
  { name: "before-native-v1", root: before, backend: "native", version: 1 },
  ...(process.env.IDEALIS_CHECKER_COMPARE_V2 === "1"
    ? ([
        {
          name: "before-native-v2",
          root: before,
          backend: "native",
          version: 2,
        },
      ] as const)
    : []),
  { name: "after-native-v1", root: after, backend: "native", version: 1 },
  { name: "after-native-v2", root: after, backend: "native", version: 2 },
  { name: "before-wasm-v1", root: before, backend: "wasm", version: 1 },
  ...(process.env.IDEALIS_CHECKER_COMPARE_V2 === "1"
    ? ([
        { name: "before-wasm-v2", root: before, backend: "wasm", version: 2 },
      ] as const)
    : []),
  { name: "after-wasm-v1", root: after, backend: "wasm", version: 1 },
  { name: "after-wasm-v2", root: after, backend: "wasm", version: 2 },
] as const;
await save("manifest.json", {
  plan: planName,
  planSha256: digest(plan),
  environment: await environment(java, jar, archive),
  snapshots,
  specs,
  inputs,
  references,
  bounds,
  warmup: 1,
  samples: 3,
  workers: 1,
  scope:
    "records: reused native JSONL host / reused Node Wasm. compile/check includes wire encoding, decoding and response validation.",
  coldScope:
    "cold: fresh native source-to-result process and maximum RSS from macOS time -l, without Node startup or TS frontend",
  driverSha256: digest(
    await readFile("evals/experiments/p0-checker/corpus.ts"),
  ),
});
const routes = await Promise.all(
  specs.map(async (spec) => ({
    ...spec,
    core:
      spec.backend === "native"
        ? startNative(resolve(spec.root, "idealis-core"))
        : await createPortableWasm(
            await readFile(join(spec.root, "idealis_core.wasm")),
          ),
  })),
);
const records = [];
const cold = [];
const observe = async (request: () => unknown) => {
  try {
    return await request();
  } catch (error) {
    if (!(error instanceof PortableCoreError)) throw error;
    return { kind: "error", code: error.code, message: error.message };
  }
};
try {
  for (const input of inputs) {
    const reference = references.find((entry) => entry.name === input.name);
    assert.ok(reference);
    for (let iteration = 0; iteration < 4; iteration++) {
      const order = routes.map(
        (_, index) => routes[(index + iteration) % routes.length],
      );
      for (const route of iteration % 2 ? order.reverse() : order) {
        assert.ok(route);
        const source = {
          source: input.source,
          config: input.config,
          ...(route.name.startsWith("before-") && route.version === 1
            ? {}
            : { irVersion: route.version }),
        };
        const start = performance.now();
        const compiled = await observe(() =>
          route.core.request({
            version: 1,
            request: { kind: "compile-tla", ...source },
          }),
        );
        const compileMs = performance.now() - start;
        assert.ok(isRecord(compiled));
        if (compiled.kind === "error") {
          assert.ok(
            input.name.startsWith("sharing-") && route.version === 1,
            JSON.stringify(compiled),
          );
          const record = {
            input: input.name,
            route: route.name,
            iteration,
            warmup: iteration === 0,
            status: "unsupported",
            compileMs,
            response: compiled,
          };
          records.push(record);
          await save(`${input.name}-${iteration}-${route.name}.json`, record);
          continue;
        }
        assert.ok(compiled.kind === "compiled" && isRecord(compiled.program));
        const program = decodeSafetyProgram(compiled.program);
        const checking = performance.now();
        const response = await route.core.request({
          version: 1,
          request: { kind: "check-safety", program, bounds },
        });
        const checkMs = performance.now() - checking;
        assert.ok(isRecord(response) && response.kind === "safety");
        const result = decodeSafetyResult(response.result);
        assert.deepEqual(
          {
            status: result.status,
            states: result.states,
            generated: result.generated,
          },
          {
            status: reference.status,
            states: reference.states,
            generated: reference.generated,
          },
        );
        const total = performance.now();
        const combined = await route.core.request({
          version: 1,
          request: { kind: "check-tla", ...source, bounds },
        });
        const sourceMs = performance.now() - total;
        assert.deepEqual(combined, response);
        const record = {
          input: input.name,
          route: route.name,
          iteration,
          warmup: iteration === 0,
          status: "complete",
          compileMs,
          checkMs,
          sourceMs,
          irBytes: Buffer.byteLength(JSON.stringify(compiled.program)),
          definitions: Array.isArray(compiled.program.definitions)
            ? compiled.program.definitions.length
            : null,
          result,
        };
        records.push(record);
        await save(`${input.name}-${iteration}-${route.name}.json`, {
          ...record,
          program: compiled.program,
        });
      }
    }
  }
  for (const input of inputs.filter(
    (input) => !input.name.startsWith("sharing-"),
  )) {
    const reference = references.find((entry) => entry.name === input.name);
    assert.ok(reference);
    const native = specs.filter((spec) => spec.backend === "native");
    for (let iteration = 0; iteration < 4; iteration++) {
      const order = native.map(
        (_, index) => native[(index + iteration) % native.length],
      );
      for (const route of iteration % 2 ? order.reverse() : order) {
        assert.ok(route);
        const request = {
          version: 1,
          request: {
            kind: "check-tla",
            source: input.source,
            config: input.config,
            bounds,
            ...(route.name.startsWith("before-") && route.version === 1
              ? {}
              : { irVersion: route.version }),
          },
        };
        const started = performance.now();
        const process = spawnSync(
          "/usr/bin/time",
          ["-l", resolve(route.root, "idealis-core")],
          { input: `${JSON.stringify(request)}\n`, encoding: "utf8" },
        );
        const wallMs = performance.now() - started;
        assert.equal(process.status, 0, process.stderr);
        const rss = /(\d+)\s+maximum resident set size/.exec(process.stderr);
        assert.ok(rss, "This cold RSS measurement requires macOS time -l");
        const response: unknown = JSON.parse(process.stdout);
        assert.ok(isRecord(response) && response.kind === "safety");
        const result = decodeSafetyResult(response.result);
        assert.deepEqual(
          {
            status: result.status,
            states: result.states,
            generated: result.generated,
          },
          {
            status: reference.status,
            states: reference.states,
            generated: reference.generated,
          },
        );
        const record = {
          input: input.name,
          route: route.name,
          iteration,
          warmup: iteration === 0,
          wallMs,
          maxRssBytes: Number(rss[1]),
          scope:
            "fresh Rust source-to-result process, Node parent JSON encoding and time wrapper included",
          result,
        };
        cold.push(record);
        await save(`${input.name}-${iteration}-${route.name}-cold.json`, {
          ...record,
          stdout: process.stdout,
          stderr: process.stderr,
        });
      }
    }
  }
  const ts = inputs.flatMap((input) =>
    ([1, 2] as const).map((irVersion) => {
      const start = performance.now();
      try {
        const program = compileTlaSafety({ ...input, irVersion });
        return {
          input: input.name,
          irVersion,
          status: "compiled",
          compileMs: performance.now() - start,
          irBytes: Buffer.byteLength(JSON.stringify(program)),
        };
      } catch (error) {
        return {
          input: input.name,
          irVersion,
          status: "unsupported",
          compileMs: performance.now() - start,
          message: String(error),
        };
      }
    }),
  );
  await save("ts-frontend.json", ts);
  await save("result.json", { status: "complete", records, cold, ts });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "p0-corpus", plan: planName, status: "complete" })}\n`,
  );
} catch (error) {
  await save("error.json", {
    status: "error",
    message: String(error),
    records,
    cold,
  });
  throw error;
} finally {
  for (const route of routes)
    if ("close" in route.core) await route.core.close();
}
