import assert from "node:assert/strict";
import {
  appendFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import type {
  PortableCommand,
  TlaSourceInput,
} from "../../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import {
  checkTlaSource,
  compileTlaSource,
} from "../../../packages/idealis/src/tla-source.js";
import { runTlc, traceInput } from "../collections-temporal/tlc.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { classifyTlc, differences } from "../tlc-differential/compare.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";

interface Input extends TlaSourceInput {
  readonly name: string;
  readonly expected: string;
  readonly tlc: string;
  readonly states?: number;
  readonly action?: string;
}

const directory = process.argv[2];
const java = process.argv[3];
const jar = process.argv[4];
const saved = process.argv[5];
const suite = process.env.IDEALIS_TLC_SUITE ?? "module-linking";
assert.ok(suite === "module-linking" || suite === "fairness-operators");
const plan =
  suite === "module-linking"
    ? "0047-module-linking"
    : "0048-fairness-operators";
assert.ok(directory && java && jar);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const base = await fingerprint();
const extra = [
  `evals/plans/${plan}.md`,
  "evals/experiments/module-linking/run.ts",
];
const sources = [
  ...base.sources,
  ...(await Promise.all(
    extra.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  )),
].sort((left, right) => left.path.localeCompare(right.path));
const sourceVersion = digest(JSON.stringify(sources));
const binaries = await Promise.all(
  [
    java,
    jar,
    "crates/idealis-core/target/release/idealis-core",
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ].map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const inputs: Input[] = JSON.parse(
  await readFile(
    `crates/rtla-compiler/tests/${suite === "module-linking" ? "modules" : "operators"}-support.json`,
    "utf8",
  ),
);
if (saved) {
  const previous = JSON.parse(
    await readFile(join(saved, "result.json"), "utf8"),
  );
  assert.equal(previous.status, "complete");
  assert.equal(previous.sourceVersion, sourceVersion);
  for (const file of previous.files)
    assert.equal(digest(await readFile(join(saved, file.path))), file.sha256);
  const manifest = JSON.parse(
    await readFile(join(saved, "manifest.json"), "utf8"),
  );
  assert.deepEqual(manifest.binaries, binaries);
  assert.deepEqual(manifest.inputs, inputs);
}
await writeFile(
  join(directory, "manifest.json"),
  JSON.stringify(
    {
      plan,
      sourceVersion,
      sources,
      binaries,
      inputs,
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
const bounds = {
  maxDepth: 100,
  maxStates: 1000,
  maxWork: 1_000_000,
  maxTemporalNodes: 10000,
};
const observe = (command: PortableCommand) => {
  try {
    return core.request(command);
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error", code: error.code, message: error.message };
  }
};
const reports: unknown[] = [];
try {
  for (const input of inputs) {
    const target = join(directory, input.name);
    await mkdir(target);
    const request = {
      source: input.source,
      config: input.config,
      ...(input.modules ? { modules: input.modules } : {}),
    };
    const command: PortableCommand = {
      version: 1,
      request: { kind: "compile-tla", ...request },
    };
    const compiled = observe(command);
    assert.deepEqual(
      await native.request(command),
      compiled,
      `${input.name}: native/Wasm compiler`,
    );
    assert.equal(
      compiled.kind === "error",
      input.expected === "unsupported",
      `${input.name}: compilation`,
    );
    const checkedCommand: PortableCommand = {
      version: 1,
      request: { kind: "check-tla", ...request, bounds },
    };
    const checked = observe(checkedCommand);
    assert.deepEqual(
      await native.request(checkedCommand),
      checked,
      `${input.name}: native/Wasm check`,
    );
    if (input.expected === "unsupported" || input.expected === "model-error")
      assert.equal(checked.kind, "error", input.name);
    else {
      assert.equal(checked.kind, "safety", input.name);
      const result = checkTlaSource(core, input, bounds);
      assert.equal(result.status, input.expected, input.name);
      if (input.action)
        assert.equal(
          result.violation?.trace[1]?.action,
          input.action,
          input.name,
        );
      if (input.states !== undefined)
        assert.equal(result.states, input.states, input.name);
    }
    const reference = await runTlc(
      java,
      jar,
      join(target, "model"),
      input,
      "Differential",
    );
    assert.equal(
      classifyTlc(reference),
      input.tlc,
      `${input.name}: TLC\n${reference.log}`,
    );
    let witness = null;
    if (compiled.kind !== "error" && input.expected !== "model-error") {
      const result = checkTlaSource(core, input, bounds);
      assert.deepEqual(
        differences({ result }, reference),
        [],
        `${input.name}: TLC result`,
      );
      if (result.violation?.trace.length) {
        const forced = traceInput(input, compileTlaSource(core, input), result);
        const replay = await runTlc(
          java,
          jar,
          join(target, "witness"),
          forced,
          "Differential",
        );
        assert.equal(
          classifyTlc(replay),
          forced.expected,
          `${input.name}: witness\n${replay.log}`,
        );
        assert.equal(replay.states, result.violation.trace.length, input.name);
        witness = {
          input: forced,
          status: classifyTlc(replay),
          states: replay.states,
        };
      }
    }
    const report = {
      name: input.name,
      compiled,
      checked,
      reference: {
        status: classifyTlc(reference),
        states: reference.states,
        generated: reference.generated,
        exit: reference.exit,
      },
      witness,
    };
    reports.push(report);
    await writeFile(
      join(target, "report.json"),
      JSON.stringify(report, null, 2),
      { flag: "wx" },
    );
    console.log(`${input.name}: matched (${input.expected})`);
  }
  if (saved)
    assert.deepEqual(
      reports,
      JSON.parse(await readFile(join(saved, "reports.json"), "utf8")),
    );
  await writeFile(
    join(directory, "reports.json"),
    JSON.stringify(reports, null, 2),
    { flag: "wx" },
  );
  for (const source of sources)
    assert.equal(digest(await readFile(source.path)), source.sha256);
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
      { status: "complete", sourceVersion, compared: reports.length, files },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: suite, status: "complete", sourceVersion })}\n`,
  );
} catch (error) {
  await writeFile(
    join(directory, "error.json"),
    JSON.stringify({ status: "error", error: String(error), reports }, null, 2),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: suite, status: "error", sourceVersion })}\n`,
  );
  throw error;
} finally {
  await native.close();
}
