import assert from "node:assert/strict";
import {
  execFileSync,
  type SpawnSyncReturns,
  spawnSync,
} from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { delimiter, join, resolve } from "node:path";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { corpus, focus } from "../rtla-compatibility/corpus.js";
import { referenceSource } from "../rtla-compatibility/reference.js";
import { createParserWasm } from "../rtla-parser/wasm.js";
import { digest } from "../usable/provenance.js";

interface Input {
  readonly name: string;
  readonly family: string;
  readonly source: string;
}
interface Catalog {
  readonly operators: readonly {
    readonly canonical: string;
    readonly fix: string;
  }[];
  readonly aliases: readonly {
    readonly spelling: string;
    readonly canonical: string;
  }[];
}
const [directory, upstream, javaHome, stable, modern, snapshot] =
  process.argv.slice(2);
assert.ok(directory && upstream && javaHome && stable && modern && snapshot);
await mkdir(directory);
await mkdir(join(directory, "cases"));
await mkdir(join(directory, "reference"));
const save = (name: string, value: unknown) =>
  writeFile(join(directory, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const catalog: Catalog = JSON.parse(
  await readFile("crates/rtla/tests/fixtures/operators.json", "utf8"),
);
const inputs: Input[] = [...focus, ...(await corpus(upstream))];
const extra = (name: string, family: string, expression: string) =>
  inputs.push({
    name,
    family,
    source: `---- MODULE Test ----\n${expression}\n====`,
  });
for (const alias of catalog.aliases) {
  const operator = catalog.operators.find(
    (operator) => operator.canonical === alias.canonical,
  );
  if (!operator || ["[", "."].includes(alias.canonical)) continue;
  const prefix = operator.fix === "prefix";
  const postfix = operator.fix === "postfix";
  const spelling = alias.spelling;
  extra(
    `${spelling}/application`,
    "operator-application",
    `Op == ${prefix ? `${spelling === "-." ? "-" : spelling} value` : postfix ? `value ${spelling}` : `left ${spelling} right`}`,
  );
  extra(
    `${spelling}/definition`,
    "operator-definition",
    `${prefix ? `${spelling} value` : postfix ? `value ${spelling}` : `left ${spelling} right`} == value`,
  );
  extra(
    `${spelling}/declaration`,
    "operator-declaration",
    `CONSTANT ${prefix ? `${spelling} _` : postfix ? `_ ${spelling}` : `_ ${spelling} _`}`,
  );
  extra(
    `${spelling}/reference`,
    "operator-reference",
    `Op == F( ${spelling} )`,
  );
}
for (const expression of [
  "LAMBDA value: value",
  "<<LAMBDA value: value>>",
  "F((LAMBDA value: value))",
  "ASSUME TRUE PROVE TRUE",
  "INSTANCE Other",
  "PROOF OBVIOUS",
  "F(\\X)",
  "F( \\X )",
])
  extra(expression, "context-controls", `Op == ${expression}`);
extra(
  "top-labeled-sequent",
  "context-controls",
  "THEOREM label::ASSUME TRUE PROVE TRUE",
);
for (const following of [
  "Next == TRUE",
  "Next(value) == value",
  "left %% right == left",
  "ASSUME TRUE",
  "THEOREM TRUE",
  "CONSTANT value",
  "RECURSIVE F(_)\nF(value) == value",
  "LOCAL Next == TRUE",
  "INSTANCE Other",
])
  extra(
    `WITH boundary/${following}`,
    "with-boundary",
    `I == INSTANCE Symbols WITH ~ <- ~\n${following}`,
  );
for (const symbol of ["\\X", "\\times", "×"])
  extra(
    `WITH product/${symbol}`,
    "with-boundary",
    `I == INSTANCE Symbols WITH F <- ${symbol}`,
  );
const completion: readonly Input[] = JSON.parse(
  await readFile("crates/rtla-compiler/tests/frontend-completion.json", "utf8"),
);
for (const input of completion)
  inputs.push({ ...input, family: `completion/${input.family}` });
const boundaries: {
  cases: readonly { id: string; source: string; expression?: string }[];
} = JSON.parse(
  await readFile(
    "fixtures/tlc-operator-sharing/analysis-boundary-manifest.json",
    "utf8",
  ),
);
for (const input of boundaries.cases) {
  inputs.push({
    name: `boundary/${input.id}`,
    family: "legacy-boundary",
    source: input.source,
  });
  if (input.expression)
    extra(
      `boundary-expression/${input.id}`,
      "legacy-boundary-expression",
      `Op == ${input.expression}`,
    );
}
const paths = execFileSync("git", [
  "ls-files",
  "-z",
  "--cached",
  "--others",
  "--exclude-standard",
  "--",
  "crates/rtla",
])
  .toString()
  .split("\0")
  .filter(Boolean)
  .concat([
    "evals/experiments/tla-coverage/syntax.ts",
    "evals/experiments/rtla-compatibility/corpus.ts",
    "evals/experiments/rtla-compatibility/reference.ts",
    "evals/experiments/rtla-parser/wasm.ts",
    "crates/rtla-compiler/tests/frontend-completion.json",
    "fixtures/tlc-operator-sharing/analysis-boundary-manifest.json",
  ]);
const sources = await Promise.all(
  [...new Set(paths)]
    .sort()
    .map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
const oracle = referenceSource
  .replace("public class SyntaxOracle", "public class CoverageOracle")
  .replace(
    "public static void main(String[] args) throws Exception {",
    "public static void main(String[] args) throws Exception { for (int index=0; index<args.length; index+=2) parse(args[index],args[index+1]); }\n  private static void parse(String path, String output) throws Exception {",
  )
  .replaceAll("args[0]", "path")
  .replaceAll("args[1]", "output");
await writeFile(join(directory, "reference/CoverageOracle.java"), oracle, {
  flag: "wx",
});
await save("inputs.json", inputs);
await save("manifest.json", {
  plan: "0070-tla-coverage",
  sources,
  referenceSourceSha256: digest(oracle),
  corpusRevision: execFileSync("git", ["rev-parse", "HEAD"], { cwd: upstream })
    .toString()
    .trim(),
  binaries: await Promise.all(
    [join(snapshot, "rtla"), join(snapshot, "rtla.wasm"), stable, modern].map(
      async (path) => ({ path, sha256: digest(await readFile(path)) }),
    ),
  ),
  scope:
    "SANY grammar acceptance and raw syntax trees; native/Wasm exact AST; fresh parser and builtin initialization per input; forward/reverse reference batches must agree. No semantic or proof verification.",
});
for (const [index, input] of inputs.entries()) {
  await mkdir(join(directory, "cases", String(index)));
  await writeFile(
    join(directory, "cases", String(index), "Input.tla"),
    input.source,
    { flag: "wx" },
  );
}
const wasm = await createParserWasm(
  await readFile(join(snapshot, "rtla.wasm")),
);
const records: {
  readonly index: number;
  readonly version: string;
  readonly name: string;
  readonly family: string;
  readonly rtla: string;
  readonly sany: string;
  readonly unicode: boolean;
}[] = [];
for (const [version, jar] of [
  ["1.7.4", stable],
  ["1.8.0", modern],
] as const) {
  const target = join(directory, "reference", version);
  await mkdir(target);
  const compiled: SpawnSyncReturns<string> = spawnSync(
    resolve(javaHome, "bin/javac"),
    [
      "-cp",
      resolve(jar),
      "-d",
      resolve(target),
      resolve(directory, "reference/CoverageOracle.java"),
    ],
    { encoding: "utf8" },
  );
  await save(`reference/${version}/compile.json`, compiled);
  assert.equal(compiled.status, 0, compiled.stderr);
  const command = (source: string) => ({
    kind: "module",
    source,
    syntaxVersion: version,
  });
  const native: SpawnSyncReturns<string> = spawnSync(
    resolve(snapshot, "rtla"),
    [],
    {
      input: `${inputs
        .map((input) => JSON.stringify(command(input.source)))
        .join("\n")}\n`,
      encoding: "utf8",
      maxBuffer: 128 * 1024 * 1024,
    },
  );
  await writeFile(join(directory, `native-${version}.jsonl`), native.stdout, {
    flag: "wx",
  });
  await save(`native-${version}-process.json`, {
    status: native.status,
    signal: native.signal,
    stderr: native.stderr,
    error: native.error,
  });
  assert.equal(native.status, 0, native.stderr);
  const actuals: unknown[] = native.stdout
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  assert.equal(actuals.length, inputs.length);
  for (const direction of ["forward", "reverse"] as const) {
    const order = inputs.map((_, index) => index);
    if (direction === "reverse") order.reverse();
    for (let start = 0; start < order.length; start += 64) {
      const chunk = order.slice(start, start + 64);
      const arguments_ = chunk.flatMap((index) => [
        resolve(directory, "cases", String(index), "Input.tla"),
        resolve(
          directory,
          "cases",
          String(index),
          `sany-${version}-${direction}.json`,
        ),
      ]);
      const result: SpawnSyncReturns<string> = spawnSync(
        resolve(javaHome, "bin/java"),
        [
          "-cp",
          `${resolve(target)}${delimiter}${resolve(jar)}`,
          "tla2sany.parser.CoverageOracle",
          ...arguments_,
        ],
        { encoding: "utf8", cwd: resolve(target), maxBuffer: 64 * 1024 * 1024 },
      );
      await save(`reference/${version}/${direction}-${start}.json`, result);
      assert.equal(result.status, 0, result.stderr);
    }
  }
  for (const [index, input] of inputs.entries()) {
    const actual = actuals[index];
    assert.ok(isRecord(actual));
    const wasmActual = wasm.request(command(input.source));
    assert.deepEqual(
      wasmActual,
      actual,
      `${version} ${input.name}: native/Wasm`,
    );
    const base = `cases/${index}`;
    await save(`${base}/rtla-${version}.json`, actual);
    await save(`${base}/wasm-${version}.json`, {
      sha256: digest(JSON.stringify(wasmActual)),
      nativeSha256: digest(JSON.stringify(actual)),
    });
    const reference: unknown = JSON.parse(
      await readFile(
        join(directory, base, `sany-${version}-forward.json`),
        "utf8",
      ),
    );
    const reverse: unknown = JSON.parse(
      await readFile(
        join(directory, base, `sany-${version}-reverse.json`),
        "utf8",
      ),
    );
    assert.deepEqual(
      reference,
      reverse,
      `${version} ${input.name}: SANY batch state`,
    );
    assert.ok(isRecord(reference) && typeof reference.kind === "string");
    const kind =
      actual.kind === "module"
        ? "accepted"
        : isRecord(actual.error)
          ? actual.error.kind
          : undefined;
    assert.ok(typeof kind === "string");
    records.push({
      index,
      version,
      name: input.name,
      family: input.family,
      rtla: kind,
      sany: reference.kind,
      unicode: [...input.source].some(
        (character) => character.charCodeAt(0) > 127,
      ),
    });
  }
}
await save("result.json", { records });
const extensions = records.filter(
  (record) =>
    record.version === "1.7.4" &&
    record.rtla !== record.sany &&
    record.sany !== "infrastructure" &&
    records.some(
      (modern) =>
        modern.index === record.index &&
        modern.version === "1.8.0" &&
        modern.sany === modern.rtla &&
        modern.rtla === record.rtla,
    ),
);
const differences = records.filter(
  (record) =>
    record.sany !== "infrastructure" &&
    (record.rtla === "accepted") !== (record.sany === "accepted") &&
    !extensions.includes(record),
);
const failures = records.filter((record) => record.sany === "infrastructure");
await save("summary.json", {
  inputs: inputs.length,
  records: records.length,
  referenceGenerationDifferences: extensions.map((record) => ({
    index: record.index,
    name: record.name,
  })),
  referenceFailures: failures,
  differences,
});
for (const source of sources)
  assert.equal(
    digest(await readFile(source.path)),
    source.sha256,
    `Source changed: ${source.path}`,
  );
assert.equal(
  differences.length,
  0,
  "Grammar differences remain; inspect saved reference trees",
);
