import assert from "node:assert/strict";
import {
  execFileSync,
  type SpawnSyncReturns,
  spawnSync,
} from "node:child_process";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, delimiter, join, resolve } from "node:path";
import { parseModule } from "tla-ts";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { createParserWasm } from "../rtla-parser/wasm.js";
import { digest } from "../usable/provenance.js";
import { corpus, focus } from "./corpus.js";
import { referenceSource } from "./reference.js";

const [directory, upstream, javaHome, jar, snapshot, referenceVersion] =
  process.argv.slice(2);
assert.ok(directory && upstream && javaHome && jar && snapshot);
assert.ok(referenceVersion === "1.7.4" || referenceVersion === "1.8.0");
const requireFocus = process.env.IDEALIS_RTLA_REQUIRE_FOCUS === "1";
await mkdir(directory);
await mkdir(join(directory, "reference"));
await mkdir(join(directory, "cases"));
const save = (path: string, value: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const inputs = [...focus, ...(await corpus(upstream))];
assert.equal(inputs.length, 368);
const java = resolve(javaHome, "bin/java");
const javac = resolve(javaHome, "bin/javac");
const adapter = resolve(directory, "reference/SyntaxOracle.java");
await writeFile(adapter, referenceSource, { flag: "wx" });
const compiled = spawnSync(
  javac,
  ["-cp", resolve(jar), "-d", resolve(directory, "reference"), adapter],
  {
    encoding: "utf8",
  },
);
await save("reference/compile.json", compiled);
assert.equal(compiled.status, 0, compiled.stderr);
const paths = execFileSync("git", [
  "ls-files",
  "-z",
  "--cached",
  "--others",
  "--exclude-standard",
  "--",
  "crates/rtla",
  "evals/experiments/rtla-compatibility",
  "tests/rtla-compatibility",
  "evals/plans/0068-rtla-compatibility.md",
])
  .toString()
  .split("\0")
  .filter(Boolean)
  .sort();
const sources = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const binaries = await Promise.all(
  [join(snapshot, "rtla"), join(snapshot, "rtla.wasm"), jar].map(
    async (path) => ({ path, sha256: digest(await readFile(path)) }),
  ),
);
await save("inputs.json", inputs);
await save("manifest.json", {
  plan: "0068-rtla-compatibility",
  requireFocus,
  sources,
  binaries,
  corpusRevision: execFileSync("git", ["rev-parse", "HEAD"], { cwd: upstream })
    .toString()
    .trim(),
  referenceVersion,
  reference:
    "parser.parse / CompilationUnit only; built-in initialization, no imports, semantics, level checks or evaluation",
  adapterSha256: digest(referenceSource),
  java: spawnSync(java, ["-version"], { encoding: "utf8" }).stderr,
  astScope:
    "Native/Wasm exact AST and errors. Official corpus comparison is acceptance only; raw SANY tree and upstream expected tree retained.",
});
await writeFile(
  join(directory, "UPSTREAM-LICENSE"),
  await readFile(join(upstream, "LICENSE")),
  { flag: "wx" },
);
const wasm = await createParserWasm(
  await readFile(join(snapshot, "rtla.wasm")),
);
const records: {
  name: string;
  family: string;
  corpusExpected: string;
  rtla: string;
  sany: string;
  tlaTs: string;
  packet: string;
}[] = [];
try {
  for (const [index, input] of inputs.entries()) {
    const path = join("cases", String(index));
    await mkdir(join(directory, path));
    await writeFile(join(directory, path, "Input.tla"), input.source, {
      flag: "wx",
    });
    const command = { kind: "module", source: input.source };
    const native: SpawnSyncReturns<string> = spawnSync(
      resolve(snapshot, "rtla"),
      [],
      {
        input: `${JSON.stringify(command)}\n`,
        encoding: "utf8",
        maxBuffer: 32 * 1024 * 1024,
      },
    );
    await save(join(path, "native-process.json"), native);
    assert.equal(native.status, 0, `${input.name}: ${native.stderr}`);
    const actual: unknown = JSON.parse(native.stdout);
    assert.deepEqual(
      wasm.request(command),
      actual,
      `${input.name}: native/Wasm`,
    );
    assert.ok(isRecord(actual));
    const rtla =
      actual.kind === "module"
        ? "accepted"
        : isRecord(actual.error)
          ? actual.error.kind
          : undefined;
    assert.ok(
      rtla === "accepted" || rtla === "syntax" || rtla === "unsupported",
    );
    const output = resolve(directory, path, "sany.json");
    const execution: SpawnSyncReturns<string> = spawnSync(
      java,
      [
        "-cp",
        `${resolve(directory, "reference")}${delimiter}${resolve(jar)}`,
        "tla2sany.parser.SyntaxOracle",
        resolve(directory, path, "Input.tla"),
        output,
      ],
      {
        encoding: "utf8",
        cwd: resolve(directory, "reference"),
        maxBuffer: 32 * 1024 * 1024,
      },
    );
    await save(join(path, "sany-process.json"), execution);
    assert.equal(
      execution.status,
      0,
      `${input.name}: oracle infrastructure: ${execution.stdout} ${execution.stderr}`,
    );
    const reference: unknown = JSON.parse(await readFile(output, "utf8"));
    assert.ok(
      isRecord(reference) &&
        (reference.kind === "accepted" ||
          reference.kind === "syntax" ||
          reference.kind === "infrastructure"),
    );
    if (input.family === "prefix-focus") {
      assert.equal(
        reference.kind,
        referenceVersion === "1.7.4" ? input.expectedStable : input.expected,
        `${input.name}: focus reference control`,
      );
      if (requireFocus)
        assert.equal(rtla, reference.kind, `${input.name}: prefix mismatch`);
    }
    let tlaTs = "accepted";
    let tlaTsError: string | undefined;
    try {
      parseModule(input.source);
    } catch (error) {
      tlaTs = "rejected";
      tlaTsError = String(error);
    }
    await save(join(path, "observation.json"), {
      actual,
      reference,
      tlaTs,
      tlaTsError,
    });
    records.push({
      name: input.name,
      family: input.family,
      corpusExpected: input.expected,
      rtla,
      sany: reference.kind,
      tlaTs,
      packet: path,
    });
    if (index % 40 === 0) console.log(index, input.name, rtla, reference.kind);
  }
  for (const source of [...sources, ...binaries])
    assert.equal(
      digest(await readFile(source.path)),
      source.sha256,
      source.path,
    );
  const matrix: Record<string, number> = {};
  for (const record of records) {
    const key = `${record.sany}/${record.rtla}`;
    matrix[key] = (matrix[key] ?? 0) + 1;
  }
  const mismatches = records.filter(
    (record) =>
      record.sany !== "infrastructure" &&
      (record.sany === "accepted") !== (record.rtla === "accepted"),
  );
  const referenceFailures = records.filter(
    (record) => record.sany === "infrastructure",
  );
  await save("result.json", {
    status: referenceFailures.length ? "partial-reference" : "complete",
    collection: "complete",
    referenceFailures,
    compatibility: "partial",
    cases: records.length,
    nativeWasmComparisons: records.length,
    matrix,
    mismatches,
    upstreamReferenceDifferences: records.filter(
      (record) => record.corpusExpected !== record.sany,
    ),
    records,
  });
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "rtla-compatibility-audit", status: referenceFailures.length ? "partial-reference" : "complete", compatibility: "partial" })}\n`,
  );
  console.log(matrix, "differences", mismatches.length);
} catch (error) {
  await save("error.json", {
    status: "error",
    message: String(error),
    records,
  });
  throw error;
}
