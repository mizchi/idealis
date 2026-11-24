import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { after, test } from "node:test";
import {
  type Baseline,
  type ChainCase,
  chainCases,
} from "../../evals/experiments/nesting-chains/cases.js";
import {
  corpora,
  fingerprint,
  sha256,
  type Surface,
} from "../../evals/experiments/nesting-chains/corpora.js";
import {
  addedCases,
  type Records0075,
  updatedCases,
} from "../../evals/experiments/nesting-chains/expectations-0075.js";
import {
  type Check,
  type Compile,
  nativeFrontend,
} from "../../evals/experiments/nesting-chains/frontends.js";
import {
  checkerLimit,
  frontendLimit,
  type Frontend,
  type Input,
  type Outcome,
  observe,
  parserLimit,
  bounds,
} from "../../evals/experiments/nesting-chains/outcome.js";
import {
  measureSize,
  type SizeSurface,
  sizeCounts,
  sizeSurfaces,
} from "../../evals/experiments/nesting-chains/sizes.js";
import { startNative } from "../../evals/experiments/usable/native.js";
import type { SafetyProgram } from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import { nativePath, wasmPath } from "../ci/tlc-build.js";
import { normalize } from "../tlc-differential/operator-sharing-support.js";

const directory = "fixtures/nesting-chains";
const readJson = async (path: string): Promise<unknown> =>
  JSON.parse(await readFile(path, "utf8"));
const baseline = (await readJson(`${directory}/baseline.json`)) as Baseline;
const cases = (await readJson(
  `${directory}/cases.json`,
)) as readonly ChainCase[];
const cases0075 = (await readJson(
  `${directory}/cases-0075.json`,
)) as readonly ChainCase[];
const records0075 = (await readJson(
  `${directory}/records-0075.json`,
)) as Records0075;
const sizes = (await readJson(`${directory}/sizes.json`)) as Record<
  string,
  Record<SizeSurface, unknown>
>;
const digests = (await readJson(`${directory}/digests.json`)) as Record<
  string,
  Record<Surface, string>
>;

const core = await createPortableWasm(await readFile(wasmPath));
const native = startNative(nativePath);
after(() => native.close());

const guarded = <Value>(
  run: () => Value,
): Value | { kind: "error"; message: string } => {
  try {
    return run();
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error", message: error.message };
  }
};

const wasm = {
  compile: async (input: Input): Promise<SafetyProgram> => {
    const reply = core.request({
      version: 1,
      request: { kind: "compile-tla", ...input },
    });
    assert.equal(reply.kind, "compiled");
    return reply.program;
  },
  check: async (program: SafetyProgram): Promise<unknown> =>
    guarded(() =>
      core.request({
        version: 1,
        request: { kind: "check-safety", program, bounds },
      }),
    ),
};
const typescript: Compile = async (input) => compileTlaSafety(input);
const nativeRust = nativeFrontend((command) => native.request(command));

interface Path {
  readonly frontend: Frontend;
  readonly label: string;
  readonly compile: Compile;
  readonly check: Check;
}
const paths: readonly Path[] = [
  {
    frontend: "ts",
    label: "TS frontend to Wasm",
    compile: typescript,
    check: wasm.check,
  },
  {
    frontend: "ts",
    label: "TS frontend to native",
    compile: typescript,
    check: nativeRust.check,
  },
  {
    frontend: "rust",
    label: "Rust frontend Wasm",
    compile: wasm.compile,
    check: wasm.check,
  },
  {
    frontend: "rust",
    label: "Rust frontend native",
    compile: nativeRust.compile,
    check: nativeRust.check,
  },
];

const comparable = (actual: Outcome, expected: Outcome): Outcome =>
  actual.stage === "result" &&
  expected.stage === "result" &&
  expected.work === null
    ? { ...actual, work: null }
    : actual;

const programs = new Map<string, Promise<SafetyProgram | null>>();
const compiled = (
  path: Path,
  name: string,
  input: Input,
): Promise<SafetyProgram | null> => {
  const key = `${path.label}\0${name}\0${input.irVersion}`;
  const found = programs.get(key);
  if (found) return found;
  const created = path.compile(input).then(
    (program) => program,
    () => null,
  );
  programs.set(key, created);
  return created;
};

const wasmReplyRefusals = new Set([
  "flat-and-128",
  "flat-or-128",
  "flat-and-129",
  "flat-or-129",
  "case-other-128",
]);
const expectedOn = (path: Path, entry: ChainCase, irVersion: 1 | 2): Outcome =>
  path.label === "Rust frontend Wasm" &&
  irVersion === 1 &&
  wasmReplyRefusals.has(entry.name)
    ? { stage: "compile", message: "Invalid compiled expression" }
    : entry.expect[path.frontend][`v${irVersion}`];

for (const entry of [...cases, ...cases0075])
  test(`nesting chains ${entry.group}: ${entry.name}`, async () => {
    for (const irVersion of [1, 2] as const) {
      const input: Input = {
        source: entry.source,
        config: entry.config,
        irVersion,
      };
      const partner =
        entry.equivalent &&
        [...cases, ...cases0075].find(
          (other) => other.name === entry.equivalent,
        );
      for (const path of paths) {
        const expected = expectedOn(path, entry, irVersion);
        const observation = await observe(path.compile, path.check, input);
        assert.deepEqual(
          comparable(observation.outcome, expected),
          expected,
          `${path.label} v${irVersion}`,
        );
        if (irVersion === 2 && partner && observation.program) {
          const other = await compiled(path, partner.name, {
            source: partner.source,
            config: partner.config,
            irVersion,
          });
          assert.ok(other, `${path.label}: ${partner.name} compiles`);
          assert.equal(
            JSON.stringify(observation.program),
            JSON.stringify(other),
            `${path.label} v${irVersion}: IR of ${entry.name} equals ${partner.name}`,
          );
        }
      }
      const [tsProgram, rustProgram, nativeProgram] = await Promise.all([
        compiled(paths[0] as Path, entry.name, input),
        compiled(paths[2] as Path, entry.name, input),
        compiled(paths[3] as Path, entry.name, input),
      ]);
      if (irVersion === 2)
        assert.deepEqual(rustProgram, nativeProgram, "Wasm and native IR v2");
      if (tsProgram && rustProgram)
        assert.deepEqual(
          normalize(tsProgram),
          normalize(rustProgram),
          `TS and Rust frontend IR v${irVersion}`,
        );
    }
  });

const find = (name: string): ChainCase => {
  const found = [...cases, ...cases0075].find((entry) => entry.name === name);
  assert.ok(found, name);
  return found;
};
const complete = (
  states: number,
  generated: number,
  work: number | null,
): Outcome => ({
  stage: "result",
  status: "complete",
  states,
  generated,
  work,
});
const refusal = (stage: "compile" | "check", text: string): Outcome => ({
  stage,
  message: text,
});

test("nesting chains fixtures follow from the recorded baselines and the plan", async () => {
  assert.deepEqual(cases, updatedCases(chainCases(baseline), records0075));
  assert.deepEqual(cases0075, addedCases(records0075));
  assert.ok(cases.length >= 200);
  assert.ok(cases0075.length >= 80);
});

const completes = (outcome: Outcome): boolean =>
  outcome.stage === "result" && outcome.status === "complete";

test("nesting chains v2 expectations count a chain as one level", () => {
  for (const frontend of ["ts", "rust"] as const) {
    for (const form of [
      "paren-and",
      "paren-or",
      "if-chain",
      "flat-and",
      "flat-or",
      "case-other",
    ])
      for (const length of [
        2, 3, 4, 24, 25, 38, 39, 40, 44, 47, 48, 49, 54, 61,
      ])
        assert.ok(
          completes(find(`${form}-${length}`).expect[frontend].v2),
          `${frontend} ${form}-${length}`,
        );
    const decoder = (outcome: Outcome): boolean =>
      outcome.stage === "check" &&
      outcome.message.startsWith("recursion limit exceeded");
    for (const form of ["flat-and", "flat-or"]) {
      assert.ok(completes(find(`${form}-100`).expect[frontend].v2));
      for (const length of [127, 128])
        assert.ok(
          decoder(find(`${form}-${length}`).expect[frontend].v2),
          `${frontend} ${form}-${length}`,
        );
    }
    assert.ok(completes(find("case-other-100").expect[frontend].v2));
    for (const length of [126, 127])
      assert.ok(
        decoder(find(`case-other-${length}`).expect[frontend].v2),
        `${frontend} case-other-${length}`,
      );
  }
  assert.deepEqual(find("flat-and-44").expect.ts.v2, complete(6, 7, 20));
  assert.deepEqual(find("paren-and-44").expect.ts.v2, complete(6, 7, 20));
  assert.deepEqual(find("paren-and-44").expect.rust.v2, complete(6, 7, 20));
});

test("nesting chains v2 expectations keep the output limit and the parser limit", () => {
  const output = refusal("compile", "compiled expression nesting exceeds 128");
  for (const frontend of ["ts", "rust"] as const) {
    assert.deepEqual(find("flat-and-129").expect[frontend].v2, output);
    assert.deepEqual(find("flat-or-129").expect[frontend].v2, output);
    assert.deepEqual(find("case-other-128").expect[frontend].v2, output);
  }
  for (const form of ["paren-and", "paren-or", "if-chain"]) {
    assert.ok(completes(find(`${form}-62`).expect.ts.v2), `ts ${form}-62`);
    for (const version of ["v1", "v2"] as const) {
      const rust = find(`${form}-62`).expect.rust[version];
      assert.ok(
        rust.stage === "compile" && rust.message.startsWith(parserLimit),
      );
    }
  }
});

test("nesting chains v1 keeps counting every level", () => {
  const limit = refusal("compile", frontendLimit);
  for (const frontend of ["ts", "rust"] as const) {
    for (const length of [39, 40, 44, 54, 61])
      assert.deepEqual(find(`paren-and-${length}`).expect[frontend].v1, limit);
    for (const length of [38, 39, 40, 44, 54, 61])
      assert.deepEqual(find(`if-chain-${length}`).expect[frontend].v1, limit);
    for (const length of [2, 3, 4, 24])
      assert.ok(
        find(`paren-and-${length}`).expect[frontend].v1.stage === "result",
      );
    assert.deepEqual(
      find("paren-and-25").expect[frontend].v1,
      refusal("check", checkerLimit),
    );
  }
});

test("nesting chains Quint output of a 44 term conjunction is checked as the flat form", async () => {
  const source = await readFile(
    `${directory}/quint/LongConjunction.tla`,
    "utf8",
  );
  assert.equal(
    createHash("sha256").update(source).digest("hex"),
    "da139c2f1461064a3197165c315c7aa136221184e67c6473e33ed2d4fa9b3954",
  );
  const expected: Record<"safety" | "broken", Outcome> = {
    safety: complete(6, 7, 20),
    broken: {
      stage: "result",
      status: "invariant-violation",
      trace: 5,
      work: null,
    },
  };
  for (const property of ["safety", "broken"] as const) {
    const config = await readFile(
      `${directory}/quint/LongConjunction-${property}.cfg`,
      "utf8",
    );
    for (const path of paths.filter((path) => path.frontend === "rust")) {
      const v1 = await observe(path.compile, path.check, {
        source,
        config,
        irVersion: 1,
      });
      assert.deepEqual(
        v1.outcome,
        refusal("compile", frontendLimit),
        `${path.label} v1`,
      );
      const v2 = await observe(path.compile, path.check, {
        source,
        config,
        irVersion: 2,
      });
      assert.deepEqual(
        comparable(v2.outcome, expected[property]),
        expected[property],
        `${path.label} ${property}`,
      );
    }
  }
});

test("nesting chains Quint output of a 54 branch IF chain is checked like TLC", async () => {
  const source = await readFile(`${directory}/quint/LongIfChain.tla`, "utf8");
  assert.equal(
    createHash("sha256").update(source).digest("hex"),
    "335653adbc2afd1efacd0628465f31a054b68c66f771e3dea0b2dacfcb2dcc98",
  );
  for (const property of ["safety", "broken"] as const) {
    const config = await readFile(
      `${directory}/quint/LongIfChain-${property}.cfg`,
      "utf8",
    );
    for (const path of paths.filter((path) => path.frontend === "rust")) {
      const v1 = await observe(path.compile, path.check, {
        source,
        config,
        irVersion: 1,
      });
      assert.deepEqual(
        v1.outcome,
        refusal("compile", frontendLimit),
        `${path.label} v1`,
      );
      const v2 = await observe(path.compile, path.check, {
        source,
        config,
        irVersion: 2,
      });
      const expected: Outcome =
        property === "safety"
          ? complete(56, 57, null)
          : {
              stage: "result",
              status: "invariant-violation",
              trace: 54,
              work: null,
            };
      assert.deepEqual(
        comparable(v2.outcome, expected),
        expected,
        `${path.label} ${property}`,
      );
    }
  }
});

test("nesting chains Quint output keeps its recorded provenance", async () => {
  const manifest = await readJson(`${directory}/quint/manifest.json`);
  assert.ok(isRecord(manifest) && Array.isArray(manifest.records));
  assert.equal(manifest.quint, "0.33.1");
  assert.equal(manifest.apalache, "0.63.0");
  assert.equal(
    manifest.apalacheSha256,
    "f2d43f2e279a657be50b2ecec3a013ba2915266008a2562b38412c3269347dc9",
  );
  for (const record of manifest.records) {
    assert.ok(isRecord(record));
    for (const [extension, key] of [
      ["tla", "tlaSha256"],
      ["qnt", "quintSha256"],
    ] as const)
      assert.equal(
        sha256(
          await readFile(
            `${directory}/quint/${String(record.name)}.${extension}`,
            "utf8",
          ),
        ),
        record[key],
      );
  }
});

const surfaces: Record<Surface, Compile> = {
  ts: typescript,
  wasm: wasm.compile,
  native: nativeRust.compile,
};
for (const name of ["ts", "wasm", "native"] as const)
  test(`nesting chains IR of the existing corpora is unchanged: ${name}`, async () => {
    const compile = surfaces[name];
    const inputs = await corpora();
    assert.deepEqual(
      inputs.map((input) => input.name),
      Object.keys(digests),
    );
    const changed: string[] = [];
    for (const input of inputs) {
      const expected = digests[input.name];
      assert.ok(expected);
      if (sha256(await fingerprint(compile, input)) !== expected[name])
        changed.push(input.name);
    }
    assert.deepEqual(changed, []);
  });

const branchLimit = "action expansion exceeds 4096 branches or 256 steps";
const rustBranchLimit = "action expansion exceeds 4096 branches";
const missing = "operator Missing";

test("nesting chains branch limit is reported before a later undefined operator", () => {
  const limit = refusal("compile", branchLimit);
  const rustLimit = refusal("compile", rustBranchLimit);
  const undefinedOperator = refusal("compile", missing);
  const everywhere = new Set([
    "or-inner-then-missing",
    "or-missing-then-inner",
    "and-chain-then-missing",
    "and-right-then-missing",
  ]);
  for (const place of ["next", "enabled", "specification"]) {
    const at = (name: string, frontend: "ts" | "rust") =>
      find(`limit-${place}-${name}`).expect[frontend].v2;
    const present = (names: readonly string[]) =>
      names.filter((name) => place === "next" || everywhere.has(name));
    for (const name of present([
      "or-inner-then-missing",
      "or-nested-left-then-missing",
      "or-nested-right-then-missing",
      "else-if-then-missing",
    ]))
      for (const frontend of ["ts", "rust"] as const)
        assert.deepEqual(at(name, frontend), limit, `${place} ${name}`);
    for (const name of present([
      "and-chain-then-missing",
      "and-flat-then-missing",
      "and-chain-in-or-then-missing",
    ])) {
      assert.deepEqual(at(name, "ts"), limit, `${place} ${name}`);
      assert.deepEqual(at(name, "rust"), rustLimit, `${place} ${name}`);
    }
    for (const name of present([
      "or-missing-then-inner",
      "or-within-limit-then-missing",
      "and-missing-then-chain",
      "and-right-then-missing",
      "and-within-limit-then-missing",
    ]))
      for (const frontend of ["ts", "rust"] as const)
        assert.deepEqual(at(name, frontend), undefinedOperator, name);
  }
});

for (const terms of sizeCounts)
  for (const surface of sizeSurfaces)
    test(`nesting chains a flat disjunction of ${terms} terms in Next keeps its outcome within the limits: ${surface}`, async () => {
      const measured = await measureSize(surface, terms);
      assert.deepEqual(measured.outcome, sizes[String(terms)]?.[surface]);
    });
