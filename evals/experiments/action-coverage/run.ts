import assert from "node:assert/strict";
import {
  appendFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { basename, join } from "node:path";
import type {
  PortableCommand,
  SafetyActionCoverage,
  TlaSourceInput,
} from "../../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { compileTlaSource } from "../../../packages/idealis/src/tla-source.js";
import { runTlc, traceInput } from "../collections-temporal/tlc.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { classifyTlc } from "../tlc-differential/compare.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";
import { assertOracle } from "../workspace-oracle/node.js";
import { coverageBounds, coverageInputs } from "./cases.js";

interface Input extends TlaSourceInput {
  readonly name: string;
  readonly expected: string;
  readonly compareTlcCoverage?: boolean;
}
const directory = process.argv[2];
const java = process.argv[3];
const jar = process.argv[4];
const saved = process.argv[5];
assert.ok(directory && java && jar);
await mkdir(directory);
const base = await fingerprint();
const sources = [
  ...base.sources,
  ...(await Promise.all(
    [
      "evals/plans/0050-action-coverage.md",
      "evals/experiments/action-coverage/cases.ts",
      "evals/experiments/action-coverage/run.ts",
    ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  )),
];
const sourceVersion = digest(JSON.stringify(sources));
const binaries = await Promise.all(
  [
    java,
    jar,
    "crates/idealis-core/target/release/idealis-core",
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const inputs: Input[] = [
  ...coverageInputs,
  ...(
    await Promise.all(
      ["finite", "modules", "operators"].map(async (name) =>
        JSON.parse(
          await readFile(
            `crates/rtla-compiler/tests/${name}-support.json`,
            "utf8",
          ),
        ),
      ),
    )
  ).flat(),
];
const limits = [
  coverageBounds,
  ...[1, 17].map((maxWork) => ({ ...coverageBounds, maxWork })),
  { ...coverageBounds, maxStates: 1 },
  { ...coverageBounds, maxDepth: 0 },
  { ...coverageBounds, maxTemporalNodes: 1 },
];
if (saved) {
  const previous = JSON.parse(
    await readFile(join(saved, "result.json"), "utf8"),
  );
  assert.equal(previous.status, "complete");
  assert.equal(previous.sourceVersion, sourceVersion);
  for (const file of previous.files)
    assert.equal(
      digest(await readFile(join(saved, file.path))),
      file.sha256,
      file.path,
    );
  const manifest = JSON.parse(
    await readFile(join(saved, "manifest.json"), "utf8"),
  );
  assertOracle(manifest.binaries, binaries);
  assertOracle(manifest.inputs, inputs);
}
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan: "0050-action-coverage",
      sourceVersion,
      sources,
      binaries,
      inputs,
      limits,
      parent: saved ?? null,
      performanceMeasurement: false,
    },
    null,
    2,
  ),
  { flag: "wx" },
);
const core = await createPortableWasm(await readFile(binaries[3]?.path ?? ""));
const native = startNative();
const observe = (command: PortableCommand) => {
  try {
    return core.request(command);
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error" as const, code: error.code, message: error.message };
  }
};
const aggregate = (rows: readonly SafetyActionCoverage[]) => {
  const totals = new Map<string, { generated: number; distinct: number }>();
  for (const row of rows) {
    const prior = totals.get(row.name) ?? { generated: 0, distinct: 0 };
    totals.set(row.name, {
      generated: prior.generated + row.generated,
      distinct: prior.distinct + row.distinct,
    });
  }
  return [...totals].sort((left, right) => left[0].localeCompare(right[0]));
};
const reports = [];
try {
  for (const input of inputs) {
    const request = {
      source: input.source,
      config: input.config,
      ...(input.modules ? { modules: input.modules } : {}),
    };
    const compiled = observe({
      version: 1,
      request: { kind: "compile-tla", ...request },
    });
    if (input.expected === "unsupported") {
      assert.equal(compiled.kind, "error", input.name);
      reports.push({ name: input.name, compiled });
      continue;
    }
    assert.equal(compiled.kind, "compiled", input.name);
    const program = compileTlaSource(core, input);
    const probes = [];
    for (const bounds of limits) {
      const measuredCommand: PortableCommand = {
        version: 1,
        request: { kind: "check-safety-coverage", program, bounds },
      };
      const normalCommand: PortableCommand = {
        version: 1,
        request: { kind: "check-safety", program, bounds },
      };
      const measured = observe(measuredCommand);
      const normal = observe(normalCommand);
      assertOracle(await native.request(measuredCommand), measured, input.name);
      assertOracle(await native.request(normalCommand), normal, input.name);
      if (measured.kind === "error") assertOracle(measured, normal, input.name);
      else {
        assert.equal(measured.kind, "safety-coverage");
        assert.equal(normal.kind, "safety");
        if (measured.kind === "safety-coverage" && normal.kind === "safety")
          assertOracle(measured.report.result, normal.result, input.name);
      }
      probes.push({ bounds, measured });
    }
    const full = probes[0]?.measured;
    assert.ok(full);
    if (input.expected === "error" || input.expected === "model-error")
      assert.equal(full.kind, "error", input.name);
    else {
      assert.equal(full.kind, "safety-coverage");
      if (full.kind === "safety-coverage")
        assert.equal(full.report.result.status, input.expected, input.name);
    }
    let reference = null;
    let witness = null;
    if (input.compareTlcCoverage !== undefined) {
      const target = join(directory, input.name);
      await mkdir(target);
      const result = await runTlc(
        java,
        jar,
        join(target, "model"),
        input,
        "Differential",
        { coverage: true },
      );
      const expected =
        input.expected === "error"
          ? "model-error"
          : input.expected === "property-violation"
            ? "liveness-violation"
            : input.expected;
      assert.equal(
        classifyTlc(result),
        expected,
        `${input.name}\n${result.log}`,
      );
      const locations = new Map<
        string,
        { name: string; first: number; second: number }
      >();
      for (const match of result.log.matchAll(
        /^(<([A-Za-z_][A-Za-z_0-9!]*) [^\n]+>): (\d+):(\d+)\s*$/gm,
      )) {
        assert.ok(match[1] && match[2]);
        locations.set(match[1], {
          name: match[2],
          first: Number(match[3]),
          second: Number(match[4]),
        });
      }
      const counts = new Map<string, { first: number; second: number }>();
      for (const row of locations.values()) {
        const old = counts.get(row.name) ?? { first: 0, second: 0 };
        counts.set(row.name, {
          first: old.first + row.first,
          second: old.second + row.second,
        });
      }
      if (
        input.compareTlcCoverage === true &&
        full.kind === "safety-coverage"
      ) {
        assert.equal(
          result.generated,
          full.report.result.generated,
          input.name,
        );
        assert.equal(result.states, full.report.result.states, input.name);
        for (const [name, row] of aggregate(full.report.coverage.initial)) {
          assert.ok(counts.has(name), `${input.name}/${name}`);
          assert.equal(
            counts.get(name)?.first,
            row.generated,
            `${input.name}/${name}`,
          );
        }
        let nextDistinct = 0;
        for (const [name, row] of aggregate(full.report.coverage.next)) {
          assertOracle(
            counts.get(name),
            { second: row.generated, first: row.distinct },
            `${input.name}/${name}`,
          );
          nextDistinct += row.distinct;
        }
        assert.equal(
          full.report.coverage.initial.reduce(
            (total, row) => total + row.distinct,
            0,
          ),
          (result.states ?? 0) - nextDistinct,
          input.name,
        );
      }
      reference = {
        status: classifyTlc(result),
        counts: [...counts],
        compared: input.compareTlcCoverage === true,
      };
      if (
        full.kind === "safety-coverage" &&
        full.report.result.violation?.trace.length &&
        [
          "invariant-violation",
          "property-violation",
          "liveness-violation",
        ].includes(full.report.result.status)
      ) {
        const forced = traceInput(input, program, full.report.result);
        const checked = await runTlc(
          java,
          jar,
          join(target, "witness"),
          forced,
          "Differential",
        );
        assert.equal(
          classifyTlc(checked),
          forced.expected,
          `${input.name} witness\n${checked.log}`,
        );
        assert.equal(
          checked.states,
          full.report.result.violation.trace.length,
          input.name,
        );
        witness = {
          input: forced,
          status: classifyTlc(checked),
          states: checked.states,
        };
      }
    }
    reports.push({ name: input.name, probes, reference, witness });
    console.log(`${input.name}: matched`);
  }
  if (saved)
    assertOracle(
      reports,
      JSON.parse(await readFile(join(saved, "reports.json"), "utf8")),
    );
  await writeFile(
    join(directory, "reports.json"),
    JSON.stringify(reports, null, 2),
    { flag: "wx" },
  );
  for (const source of sources)
    assert.equal(
      digest(await readFile(source.path)),
      source.sha256,
      source.path,
    );
  for (const binary of binaries)
    assert.equal(
      digest(await readFile(binary.path)),
      binary.sha256,
      binary.path,
    );
  const files: { path: string; sha256: string }[] = [];
  const collect = async (relative = ""): Promise<void> => {
    for (const entry of await readdir(join(directory, relative), {
      withFileTypes: true,
    })) {
      const path = join(relative, entry.name);
      if (entry.isDirectory()) await collect(path);
      else
        files.push({
          path,
          sha256: digest(await readFile(join(directory, path))),
        });
    }
  };
  await collect();
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      { status: "complete", sourceVersion, cases: reports.length, files },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "action-coverage", status: "complete", sourceVersion })}\n`,
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify(
      { status: "error", message: String(error), reports },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "action-coverage", status: "error", sourceVersion })}\n`,
  );
  throw error;
} finally {
  await native.close();
}
