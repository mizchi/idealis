import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { expandTemporalTemplates } from "../../evals/experiments/tlc-differential/expand-temporal.js";
import type {
  PortableCommand,
  SafetyProgram,
  TlaSourceOptions,
} from "../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

const bounds = { maxDepth: 8, maxStates: 100, maxWork: 10_000 };
const fixtureRoot = "fixtures/tlc-operator-sharing";
const reuseRoot = "evals/runs/0046-finite-verified";
const wasm = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const capture = (run: () => unknown): unknown => {
  try {
    return run();
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error", code: error.code, message: error.message };
  }
};
const native = (request: PortableCommand["request"]): unknown => {
  const result = spawnSync(
    "crates/idealis-core/target/release/idealis-core",
    [],
    {
      input: `${JSON.stringify({ version: 1, request })}\n`,
      encoding: "utf8",
      timeout: 1000,
      maxBuffer: 16 * 1024 * 1024,
    },
  );
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
};
const normalize = (input: unknown): unknown => {
  const value = expandTemporalTemplates(input);
  if (Array.isArray(value)) return value.map(normalize);
  if (!isRecord(value)) return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(
        ([key, entry]) =>
          !(key === "subscript" && entry === null) &&
          !(
            [
              "fairness",
              "initialProperties",
              "actionProperties",
              "properties",
            ].includes(key) &&
            Array.isArray(entry) &&
            entry.length === 0
          ),
      )
      .map(([key, entry]) => [key, normalize(entry)]),
  );
};
const source = (
  declarations: string,
  safe: string,
  next = "UNCHANGED x",
  property = false,
): TlaSourceOptions => ({
  source: `---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\n${declarations}\nInit == x = 0\nNext == ${next}\nSafe == ${safe}\n====`,
  config: `INIT Init\nNEXT Next\n${property ? "PROPERTY" : "INVARIANT"} Safe\nCHECK_DEADLOCK FALSE`,
  irVersion: 2,
});
const compile = (
  input: TlaSourceOptions,
  frontend: "TS" | "native" | "Wasm",
): unknown =>
  frontend === "TS"
    ? capture(() => ({ kind: "compiled", program: compileTlaSafety(input) }))
    : frontend === "native"
      ? native({ kind: "compile-tla", ...input })
      : capture(() =>
          wasm.request({
            version: 1,
            request: { kind: "compile-tla", ...input },
          }),
        );
const programOf = (value: unknown): SafetyProgram => {
  assert.ok(isRecord(value));
  assert.equal(value.kind, "compiled", JSON.stringify(value));
  assert.ok(isRecord(value.program));
  return value.program as unknown as SafetyProgram;
};
const check = (program: SafetyProgram) => {
  const request = { kind: "check-safety" as const, program, bounds };
  const result = capture(() => wasm.request({ version: 1, request }));
  assert.deepEqual(native(request), result);
  return result;
};
const pairedChecker = () => {
  let nativeWitness:
    | { readonly program: SafetyProgram; readonly response: unknown }
    | undefined;
  return (
    program: SafetyProgram,
    frontend: "TS" | "native" | "Wasm",
  ): unknown => {
    if (frontend === "Wasm" && nativeWitness) {
      assert.deepEqual(
        normalize(program),
        normalize(nativeWitness.program),
        "native/Wasm compiled IR exact parity",
      );
      return nativeWitness.response;
    }
    const response = check(program);
    if (frontend === "native") nativeWitness = { program, response };
    return response;
  };
};

const assertComplete = (result: unknown) => {
  assert.ok(isRecord(result));
  assert.equal(result.kind, "safety", JSON.stringify(result));
  assert.ok(isRecord(result.result));
  assert.equal(result.result.status, "complete");
  assert.equal(result.result.reason, null);
  assert.equal(result.result.states, 1);
  assert.equal(result.result.generated, 2);
  assert.equal(result.result.depth, 0);
  assert.equal(result.result.violation, null);
};
const noWitness = {
  kind: "error",
  code: "protocol",
  message: "CHOOSE: no element of the domain satisfied the predicate",
};
const hasNode = (
  value: unknown,
  predicate: (node: Record<string, unknown>) => boolean,
): boolean =>
  Array.isArray(value)
    ? value.some((entry) => hasNode(entry, predicate))
    : isRecord(value)
      ? predicate(value) ||
        Object.values(value).some((entry) => hasNode(entry, predicate))
      : false;

interface CallActualCase {
  readonly id: string;
  readonly analysis: string;
  readonly demanded: boolean;
  readonly input: TlaSourceOptions;
  readonly expected: { readonly kind: "compiled" | "error" };
  readonly assertions: readonly {
    readonly pointer: string;
    readonly value: unknown;
  }[];
  readonly contains: readonly Record<string, unknown>[];
  readonly absent: readonly {
    readonly pointer: string;
    readonly node: Record<string, unknown>;
  }[];
}
const callActualCases: readonly CallActualCase[] = JSON.parse(
  await readFile(`${fixtureRoot}/analysis-call-actual-cases.json`, "utf8"),
).cases;
const atPointer = (value: unknown, pointer: string): unknown =>
  pointer
    .split("/")
    .slice(1)
    .reduce<unknown>((current, key) => {
      assert.ok(isRecord(current) || Array.isArray(current), pointer);
      return Reflect.get(current, key);
    }, value);
const matchesNode = (value: unknown, expected: unknown): boolean => {
  if (Array.isArray(expected))
    return (
      Array.isArray(value) &&
      value.length === expected.length &&
      expected.every((entry, index) => matchesNode(value[index], entry))
    );
  if (isRecord(expected))
    return (
      isRecord(value) &&
      Object.entries(expected).every(([key, entry]) =>
        matchesNode(value[key], entry),
      )
    );
  return value === expected;
};
for (const entry of callActualCases) {
  for (const frontend of ["TS", "native", "Wasm"] as const)
    test(`analysis parity call actual ${entry.id}: ${frontend}`, () => {
      const output = compile(entry.input, frontend);
      if (entry.expected.kind === "error") {
        assert.deepEqual(
          output,
          entry.expected,
          `${entry.id}: ${frontend} refusal`,
        );
        return;
      }
      const program = programOf(output);
      assert.equal(program.version, 2);
      for (const assertion of entry.assertions)
        assert.deepEqual(
          normalize(atPointer(program, assertion.pointer)),
          assertion.value,
          `${entry.id}: ${frontend} ${assertion.pointer}`,
        );
      for (const node of entry.contains)
        assert.ok(
          hasNode(program, (value) => matchesNode(value, node)),
          `${entry.id}: ${frontend} expected IR node ${JSON.stringify(node)}`,
        );
      for (const assertion of entry.absent)
        assert.equal(
          hasNode(atPointer(program, assertion.pointer), (value) =>
            matchesNode(value, assertion.node),
          ),
          false,
          `${entry.id}: ${frontend} unexpected IR node ${JSON.stringify(assertion.node)}`,
        );
      if (frontend !== "Wasm") assertComplete(check(program));
    });
  if (entry.expected.kind === "compiled")
    test(`analysis parity call actual ${entry.id}: normalized v2 IR`, () => {
      const programs = (["TS", "native", "Wasm"] as const).map((frontend) =>
        programOf(compile(entry.input, frontend)),
      );
      const tsResult = check(programs[0] as SafetyProgram);
      const rustResult = check(programs[1] as SafetyProgram);
      assertComplete(tsResult);
      assertComplete(rustResult);
      assert.deepEqual(tsResult, rustResult);
      assert.deepEqual(
        normalize(programs[1]),
        normalize(programs[0]),
        `${entry.id}: native/TS normalized v2 IR`,
      );
      assert.deepEqual(
        normalize(programs[2]),
        normalize(programs[0]),
        `${entry.id}: Wasm/TS normalized v2 IR`,
      );
    });
}
test("analysis parity call actual sweep covers unused and demanded formals", () => {
  for (const analysis of ["membership", "target", "generative", "free-old"])
    for (const demanded of [false, true])
      assert.ok(
        callActualCases.some(
          (entry) => entry.analysis === analysis && entry.demanded === demanded,
        ),
        `${analysis}: demanded=${demanded}`,
      );
});

const known = [
  {
    id: "CASE condition is a value position",
    input: source(
      "F(p) == ENABLED (CASE UNCHANGED p -> UNCHANGED x [] OTHER -> UNCHANGED x)",
      "F(0)",
    ),
  },
  {
    id: "notin formal membership",
    input: source(String.raw`F(p) == x \notin p`, "~F(Nat)"),
  },
  {
    id: "notin captured membership",
    input: source(String.raw`F(p) == LET G == x \notin p IN G`, "~F(Nat)"),
  },
  {
    id: "notin function head membership",
    input: source(
      String.raw`F(p) == LET f[i \in {0}] == i \notin p IN f[0]`,
      "~F(Nat)",
    ),
  },
  {
    id: "notin function construction membership",
    input: source(
      String.raw`F(p) == LET f == [i \in {0} |-> i \notin p] IN f[0]`,
      "~F(Nat)",
    ),
  },
] as const;
for (const entry of known) {
  const verify = pairedChecker();
  for (const frontend of ["TS", "native", "Wasm"] as const) {
    test(`analysis parity controls ${entry.id}: ${frontend}`, () => {
      const program = programOf(compile(entry.input, frontend));
      assertComplete(verify(program, frontend));
    });
  }
}

const chooseFreeOld = [
  "CHOOSE n \\in @: TRUE",
  "CHOOSE n \\in {9}: @ = 9",
] as const;
for (const [index, choose] of chooseFreeOld.entries()) {
  for (const unused of [false, true]) {
    const input = source(
      "",
      `[<<{9}>> EXCEPT ![1] = LET Pick == ${choose} IN ${unused ? "TRUE" : "Pick = 9"}][1]`,
    );
    for (const frontend of ["TS", "native", "Wasm"] as const)
      test(`analysis parity controls CHOOSE free old ${index === 0 ? "domain" : "predicate"} ${unused ? "unused" : "used"}: ${frontend}`, () => {
        const result = check(programOf(compile(input, frontend)));
        if (!unused && index === 1)
          assert.deepEqual(result, {
            kind: "error",
            code: "protocol",
            message: "Incomparable TLA values",
          });
        else assertComplete(result);
      });
  }
}

for (const form of ["head", "explicit"] as const) {
  const definition =
    form === "head"
      ? String.raw`f[i \in @] == i`
      : String.raw`f == [i \in @ |-> i]`;
  const uses = [
    "TRUE",
    "f[9] = 9",
    "ENABLED (x' = f[9])",
    `LET Identity(a) == LET ${definition} IN f[9] IN Identity(0) = 9`,
  ];
  for (const [index, use] of uses.entries())
    test(`analysis parity controls function domain free old ${form} ${index}`, () => {
      const input = source(
        "",
        `[<<{9}>> EXCEPT ![1] = ${index === 3 ? use : `LET ${definition} IN ${use}`}][1]`,
      );
      const verify = pairedChecker();
      for (const frontend of ["TS", "native", "Wasm"] as const)
        assertComplete(verify(programOf(compile(input, frontend)), frontend));
    });
  for (const [label, parameter, domain, safe, element] of [
    ["target", "v", "IF ENABLED (UNCHANGED v) THEN {0} ELSE {}", "F(x)", 0],
    ["membership", "p", String.raw`{IF x \in p THEN 1 ELSE 2}`, "F(Nat)", 1],
  ] as const)
    test(`analysis parity controls function domain ${form} ${label}`, () => {
      const binding =
        form === "head"
          ? `f[i \\in (${domain})] == i`
          : `f == [i \\in (${domain}) |-> i]`;
      const input = source(
        `F(${parameter}) == LET ${binding} IN f[${element}] = ${element}`,
        safe,
      );
      const programs = (["TS", "native", "Wasm"] as const).map((frontend) =>
        programOf(compile(input, frontend)),
      );
      assert.deepEqual(normalize(programs[0]), normalize(programs[1]));
      assert.deepEqual(normalize(programs[0]), normalize(programs[2]));
      for (const program of programs.slice(0, 2))
        assertComplete(check(program));
    });
}

const chooseContexts = [
  {
    id: "domain caller binder",
    declarations: String.raw`D(p) == CHOOSE n \in p: TRUE`,
    safe: String.raw`\A n \in {9}: D({n}) = n`,
  },
  {
    id: "predicate caller binder",
    declarations: String.raw`P(p) == CHOOSE n \in {4}: p`,
    safe: String.raw`\A n \in {9}: P(n = 9) = 4`,
  },
  {
    id: "domain caller old actual",
    declarations: "I(p) == [<<4>> EXCEPT ![1] = p][1]",
    safe: String.raw`[<<9>> EXCEPT ![1] = I(CHOOSE n \in {@}: TRUE)][1] = 9`,
  },
  {
    id: "predicate caller old actual",
    declarations: "I(p) == [<<4>> EXCEPT ![1] = p][1]",
    safe: String.raw`[<<9>> EXCEPT ![1] = I(CHOOSE n \in {9}: n = @)][1] = 9`,
  },
  {
    id: "unused domain caller old actual",
    declarations: "I(p) == [<<4>> EXCEPT ![1] = 9][1]",
    safe: String.raw`[<<9>> EXCEPT ![1] = I(CHOOSE n \in {@}: TRUE)][1] = 9`,
  },
  {
    id: "unused predicate caller old actual",
    declarations: "I(p) == [<<4>> EXCEPT ![1] = 9][1]",
    safe: String.raw`[<<9>> EXCEPT ![1] = I(CHOOSE n \in {9}: n = @)][1] = 9`,
  },
  {
    id: "no witness",
    declarations: "",
    safe: String.raw`(CHOOSE n \in {9}: FALSE) = 9`,
    error: noWitness,
  },
  {
    id: "empty domain",
    declarations: "",
    safe: String.raw`(CHOOSE n \in {}: TRUE) = 9`,
    error: noWitness,
  },
  {
    id: "lazy no witness branch",
    declarations: "",
    safe: String.raw`IF TRUE THEN TRUE ELSE (CHOOSE n \in {9}: FALSE) = 9`,
  },
  {
    id: "lazy empty branch",
    declarations: "",
    safe: String.raw`IF TRUE THEN TRUE ELSE (CHOOSE n \in {}: TRUE) = 9`,
  },
  {
    id: "unused empty actual",
    declarations: "I(p) == TRUE",
    safe: String.raw`I(CHOOSE n \in {}: TRUE)`,
  },
  {
    id: "unused no witness actual",
    declarations: "I(p) == TRUE",
    safe: String.raw`I(CHOOSE n \in {9}: FALSE)`,
  },
] as const;
for (const entry of chooseContexts) {
  const verify = pairedChecker();
  for (const frontend of ["TS", "native", "Wasm"] as const)
    test(`analysis parity controls CHOOSE ${entry.id}: ${frontend}`, () => {
      const program = programOf(
        compile(source(entry.declarations, entry.safe), frontend),
      );
      assert.ok(
        hasNode(program, (node) => node.op === "choose"),
        "CHOOSE node must survive lowering even in unused actuals and branches",
      );
      const result = verify(program, frontend);
      if ("error" in entry) assert.deepEqual(result, entry.error);
      else assertComplete(result);
    });
}

const chooseShapes = [
  { id: "scalar", pattern: "n", value: "4" },
  { id: "tuple", pattern: "<<n,m>>", value: "<<4,1>>" },
  { id: "nested", pattern: "n", value: "CHOOSE m \\in {4}: TRUE" },
] as const;
const chooseProbes = [
  {
    id: "binder",
    declarations: "I(p) == p = 9",
    expression: "I(p)",
    actual: "n",
    wrapper: (value: string) => `\\A n \\in {9}: ${value}`,
  },
  {
    id: "target",
    declarations: "I(p) == ENABLED (UNCHANGED p)",
    expression: "I(p)",
    actual: "x",
    wrapper: (value: string) => value,
  },
  {
    id: "membership",
    declarations: String.raw`I(p) == 0 \in p`,
    expression: "I(p)",
    actual: "Nat",
    wrapper: (value: string) => value,
  },
  {
    id: "old actual",
    declarations: "I(p) == [<<4>> EXCEPT ![1] = p][1] = 9",
    expression: "I(p)",
    actual: "@",
    wrapper: (value: string) => `[<<9>> EXCEPT ![1] = ${value}][1]`,
  },
] as const;
for (const shape of chooseShapes)
  for (const position of ["domain", "predicate"] as const)
    for (const probe of chooseProbes) {
      const verify = pairedChecker();
      for (const frontend of ["TS", "native", "Wasm"] as const)
        test(`analysis parity controls CHOOSE ${shape.id} ${position} ${probe.id}: ${frontend}`, () => {
          const domain =
            position === "domain"
              ? `(IF ${probe.expression} THEN {${shape.value}} ELSE {})`
              : `{${shape.value}}`;
          const predicate =
            position === "predicate" ? probe.expression : "TRUE";
          const choose = `(CHOOSE ${shape.pattern} \\in ${domain}: ${predicate})`;
          const safe = probe.wrapper(`C(${probe.actual}) = (${shape.value})`);
          const program = programOf(
            compile(
              source(`${probe.declarations}\nC(p) == ${choose}`, safe),
              frontend,
            ),
          );
          assert.ok(
            hasNode(
              program,
              (node) =>
                node.op === "choose" &&
                isRecord(node.binding) &&
                isRecord(node.binding.domain) &&
                isRecord(node.body),
            ),
            "domain and predicate must be represented separately",
          );
          assertComplete(verify(program, frontend));
        });
    }

for (const shape of chooseShapes)
  for (const position of ["domain", "predicate"] as const)
    for (const frontend of ["TS", "native", "Wasm"] as const)
      test(`analysis parity controls CHOOSE ${shape.id} ${position} LET free old: ${frontend}`, () => {
        const domain =
          position === "domain"
            ? `(IF @ = 9 THEN {${shape.value}} ELSE {})`
            : `{${shape.value}}`;
        const predicate = position === "predicate" ? "@ = 9" : "TRUE";
        const input = source(
          "",
          `[<<9>> EXCEPT ![1] = LET Pick == CHOOSE ${shape.pattern} \\in ${domain}: ${predicate} IN TRUE][1]`,
        );
        assertComplete(check(programOf(compile(input, frontend))));
      });

const actions = [
  {
    id: "4096 paths",
    declarations: "B(a) == (x' = a) \\/ (x' = a)",
    next: Array(12).fill("B(0)").join(" /\\ "),
    accepted: true,
  },
  {
    id: "8192 paths",
    declarations: "B(a) == (x' = a) \\/ (x' = a)",
    next: Array(13).fill("B(0)").join(" /\\ "),
    accepted: false,
  },
  ...[256, 257].map((steps) => ({
    id: `${steps} direct steps`,
    declarations: "",
    next: Array(steps).fill("x=0").join("/\\"),
    accepted: steps === 256,
  })),
  ...[254, 255].map((guards) => ({
    id: `${guards + 2} wrapper steps`,
    declarations: "B(a) == x' = a",
    next: ["B(0)", ...Array(guards).fill("x=0")].join("/\\"),
    accepted: guards === 254,
  })),
  {
    id: "actual ENABLED excluded from action dimensions",
    declarations: "B(a) == x' = (IF a THEN 0 ELSE 0)",
    next: ["B(ENABLED (UNCHANGED x))", ...Array(254).fill("x=0")].join("/\\"),
    accepted: true,
  },
  {
    id: "capture ENABLED excluded from action dimensions",
    declarations: "B(a) == LET G(b) == x' = (IF a THEN b ELSE b) IN G(0)",
    next: ["B(ENABLED (UNCHANGED x))", ...Array(252).fill("x=0")].join("/\\"),
    accepted: true,
  },
] as const;
for (const entry of actions)
  test(`analysis parity controls action ${entry.id}`, () => {
    const input = source(entry.declarations, "TRUE", entry.next);
    const outputs = (["TS", "native", "Wasm"] as const).map((frontend) =>
      compile(input, frontend),
    );
    if (entry.accepted) {
      const programs = outputs.map(programOf);
      assert.deepEqual(normalize(programs[0]), normalize(programs[1]));
      assert.deepEqual(normalize(programs[0]), normalize(programs[2]));
    } else
      for (const output of outputs)
        assert.deepEqual(output, {
          kind: "error",
          code: "protocol",
          message:
            "Unsupported TLA input: action expansion exceeds 4096 branches or 256 steps",
        });
  });
for (const formula of ["p", "[]p"] as const)
  test(`analysis parity controls PROPERTY helper ${formula}`, () => {
    const input = source(`F(p) == ${formula}`, "F(x = 0)", "UNCHANGED x", true);
    const programs = (["TS", "native", "Wasm"] as const).map((frontend) =>
      programOf(compile(input, frontend)),
    );
    assert.deepEqual(normalize(programs[0]), normalize(programs[1]));
    assert.deepEqual(normalize(programs[0]), normalize(programs[2]));
    for (const program of programs.slice(0, 2)) {
      assert.ok(
        hasNode(
          program,
          (node) =>
            node.op === "frame" &&
            isRecord(node.body) &&
            node.body.op === "parameter",
        ),
        "PROPERTY frame must survive atom/always classification",
      );
      assertComplete(check(program));
    }
    assert.equal(
      programs[0]?.initialProperties?.length ?? 0,
      formula === "p" ? 1 : 0,
    );
    assert.equal(
      programs[0]?.invariants.length ?? 0,
      formula === "[]p" ? 1 : 0,
    );
  });

for (const irVersion of [undefined, 1] as const)
  test(`analysis parity controls v1 finite CHOOSE remains frontend specific ${irVersion ?? "default"}`, () => {
    const input = source("", String.raw`(CHOOSE n \in {9}: TRUE) = 9`);
    const versioned = {
      source: input.source,
      config: input.config,
      ...(irVersion === undefined ? {} : { irVersion }),
    };
    assert.deepEqual(compile(versioned, "TS"), {
      kind: "error",
      code: "protocol",
      message: "Unsupported TLA input: expression Choose",
    });
    const programs = (["native", "Wasm"] as const).map((frontend) =>
      programOf(compile(versioned, frontend)),
    );
    for (const program of programs.slice(0, 1)) {
      assert.equal(program.version, 1);
      assertComplete(check(program));
    }
    assert.deepEqual(normalize(programs[0]), normalize(programs[1]));
    assert.deepEqual(
      compile(versioned, "native"),
      compile({ ...input, irVersion: 1 }, "native"),
    );
  });

const newChooseV1 = [
  {
    id: "domain caller binder",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\nD(p) == CHOOSE n \\in p: TRUE\nInit == x = 0\nNext == UNCHANGED x\nSafe == \\A n \\in {9}: D({n}) = n\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "a37a0efedf41c98b36239cf519305249157841a79fbc68c6a709bc8038880b9e",
      Wasm: "a37a0efedf41c98b36239cf519305249157841a79fbc68c6a709bc8038880b9e",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "predicate caller binder",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\nP(p) == CHOOSE n \\in {4}: p\nInit == x = 0\nNext == UNCHANGED x\nSafe == \\A n \\in {9}: P(n = 9) = 4\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "747029143763f3ffe451f704e89c52c5826d506b7d374b1607982b041311614e",
      Wasm: "747029143763f3ffe451f704e89c52c5826d506b7d374b1607982b041311614e",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "domain caller old actual",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\nI(p) == [<<4>> EXCEPT ![1] = p][1]\nInit == x = 0\nNext == UNCHANGED x\nSafe == [<<9>> EXCEPT ![1] = I(CHOOSE n \\in {@}: TRUE)][1] = 9\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "0d7870fbc0a6261478b5658cf387edd6d87beaceca50fb7f928c2385ec2462d6",
      Wasm: "0d7870fbc0a6261478b5658cf387edd6d87beaceca50fb7f928c2385ec2462d6",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "predicate caller old actual",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\nI(p) == [<<4>> EXCEPT ![1] = p][1]\nInit == x = 0\nNext == UNCHANGED x\nSafe == [<<9>> EXCEPT ![1] = I(CHOOSE n \\in {9}: n = @)][1] = 9\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "f41e1c59b3de92b3e7919f58dbc613aa019e5dbdc6847e929fd6027f598473cc",
      Wasm: "f41e1c59b3de92b3e7919f58dbc613aa019e5dbdc6847e929fd6027f598473cc",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "unused domain caller old actual",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\nI(p) == [<<4>> EXCEPT ![1] = 9][1]\nInit == x = 0\nNext == UNCHANGED x\nSafe == [<<9>> EXCEPT ![1] = I(CHOOSE n \\in {@}: TRUE)][1] = 9\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "add74808989ac4ffd44c7f8d577e69f5f5b918fd13fa0ecf046380b1f9ec71ef",
      Wasm: "add74808989ac4ffd44c7f8d577e69f5f5b918fd13fa0ecf046380b1f9ec71ef",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "unused predicate caller old actual",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\nI(p) == [<<4>> EXCEPT ![1] = 9][1]\nInit == x = 0\nNext == UNCHANGED x\nSafe == [<<9>> EXCEPT ![1] = I(CHOOSE n \\in {9}: n = @)][1] = 9\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "add74808989ac4ffd44c7f8d577e69f5f5b918fd13fa0ecf046380b1f9ec71ef",
      Wasm: "add74808989ac4ffd44c7f8d577e69f5f5b918fd13fa0ecf046380b1f9ec71ef",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "no witness",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\n\nInit == x = 0\nNext == UNCHANGED x\nSafe == (CHOOSE n \\in {9}: FALSE) = 9\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "1f3905edd946a20563042f927e24f547ac1e40fdcd3de7cad3375ae501167ffe",
      Wasm: "1f3905edd946a20563042f927e24f547ac1e40fdcd3de7cad3375ae501167ffe",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "empty domain",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\n\nInit == x = 0\nNext == UNCHANGED x\nSafe == (CHOOSE n \\in {}: TRUE) = 9\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "a172d62bbeb3382b120908a7035efec2f7dce0f854814410fc66fd104fe81736",
      Wasm: "a172d62bbeb3382b120908a7035efec2f7dce0f854814410fc66fd104fe81736",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "lazy no witness branch",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\n\nInit == x = 0\nNext == UNCHANGED x\nSafe == IF TRUE THEN TRUE ELSE (CHOOSE n \\in {9}: FALSE) = 9\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "788a8dd85193bf1ca36608927c4c0f2926956262c212c489a8ff8941ba59be55",
      Wasm: "788a8dd85193bf1ca36608927c4c0f2926956262c212c489a8ff8941ba59be55",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "lazy empty branch",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\n\nInit == x = 0\nNext == UNCHANGED x\nSafe == IF TRUE THEN TRUE ELSE (CHOOSE n \\in {}: TRUE) = 9\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "b9a5b70970aaea9e006ee3d8dfa82b328ab6cccd015baead39338d7f6ec81fae",
      Wasm: "b9a5b70970aaea9e006ee3d8dfa82b328ab6cccd015baead39338d7f6ec81fae",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "unused empty actual",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\nI(p) == TRUE\nInit == x = 0\nNext == UNCHANGED x\nSafe == I(CHOOSE n \\in {}: TRUE)\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "5f7cafaad5a5c503b517d42af23fd9b2d7ce240d150d4610a0797857313a254a",
      Wasm: "5f7cafaad5a5c503b517d42af23fd9b2d7ce240d150d4610a0797857313a254a",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "unused no witness actual",
    input: {
      source:
        "---- MODULE AnalysisControl ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets\nVARIABLE x\nI(p) == TRUE\nInit == x = 0\nNext == UNCHANGED x\nSafe == I(CHOOSE n \\in {9}: FALSE)\n====",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "5f7cafaad5a5c503b517d42af23fd9b2d7ce240d150d4610a0797857313a254a",
      Wasm: "5f7cafaad5a5c503b517d42af23fd9b2d7ce240d150d4610a0797857313a254a",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "sharing-choose-domain-caller-step3f",
    input: {
      source:
        "---- MODULE Differential ----\nEXTENDS Integers, Sequences\nVARIABLE x\nD(p) == CHOOSE n \\in p: TRUE\nInit == x = 0\nNext == UNCHANGED x\nSafe == \\A n \\in {9}: D({n}) = n\n====\n",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "b5092ded1e987a593f8081192d8dac6a94acda988495812191ff4f71f4c80950",
      Wasm: "b5092ded1e987a593f8081192d8dac6a94acda988495812191ff4f71f4c80950",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "sharing-choose-predicate-caller-step3f",
    input: {
      source:
        "---- MODULE Differential ----\nEXTENDS Integers, Sequences\nVARIABLE x\nP(p) == CHOOSE n \\in {4}: p\nInit == x = 0\nNext == UNCHANGED x\nSafe == \\A n \\in {9}: P(n = 9) = 4\n====\n",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "6426ba34e295ee7d424789f6cf8fd05edc2bef4b54bb4c77ed8486b5b058f977",
      Wasm: "6426ba34e295ee7d424789f6cf8fd05edc2bef4b54bb4c77ed8486b5b058f977",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "sharing-choose-old-domain-argument-step3f",
    input: {
      source:
        "---- MODULE Differential ----\nEXTENDS Integers, Sequences\nVARIABLE x\nI(p) == [<<4>> EXCEPT ![1] = p][1]\nInit == x = 0\nNext == UNCHANGED x\nSafe == [<<9>> EXCEPT ![1] = I(CHOOSE n \\in {@}: TRUE)][1] = 9\n====\n",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "d794bc025af2d733d8e6f139e44c5dfbb3488f044137343c2970d3a27694d392",
      Wasm: "d794bc025af2d733d8e6f139e44c5dfbb3488f044137343c2970d3a27694d392",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "sharing-choose-old-predicate-argument-step3f",
    input: {
      source:
        "---- MODULE Differential ----\nEXTENDS Integers, Sequences\nVARIABLE x\nI(p) == [<<4>> EXCEPT ![1] = p][1]\nInit == x = 0\nNext == UNCHANGED x\nSafe == [<<9>> EXCEPT ![1] = I(CHOOSE n \\in {9}: n = @)][1] = 9\n====\n",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "2971f4a04289b74e6503c29677a8f5603a15a419110f29611a5f845b7f1c6502",
      Wasm: "2971f4a04289b74e6503c29677a8f5603a15a419110f29611a5f845b7f1c6502",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "sharing-choose-domain-caller-step3f-violation",
    input: {
      source:
        "---- MODULE Differential ----\nEXTENDS Integers, Sequences\nVARIABLE x\nD(p) == CHOOSE n \\in p: TRUE\nInit == x = 0\nNext == UNCHANGED x\nSafe == \\A n \\in {9}: D({n}) = 0\n====\n",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "7b6031dd2eedf135b624cfa72367a57cac425a51752233bec94be628c5aee613",
      Wasm: "7b6031dd2eedf135b624cfa72367a57cac425a51752233bec94be628c5aee613",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "sharing-choose-predicate-caller-step3f-violation",
    input: {
      source:
        "---- MODULE Differential ----\nEXTENDS Integers, Sequences\nVARIABLE x\nP(p) == CHOOSE n \\in {4}: p\nInit == x = 0\nNext == UNCHANGED x\nSafe == \\A n \\in {9}: P(n = 9) = 0\n====\n",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "caae8a1b0f41dde5272daa46ba7d3901cbf8a0e2af38db09cdeb74701cd88d99",
      Wasm: "caae8a1b0f41dde5272daa46ba7d3901cbf8a0e2af38db09cdeb74701cd88d99",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "sharing-choose-old-domain-argument-step3f-violation",
    input: {
      source:
        "---- MODULE Differential ----\nEXTENDS Integers, Sequences\nVARIABLE x\nI(p) == [<<4>> EXCEPT ![1] = p][1]\nInit == x = 0\nNext == UNCHANGED x\nSafe == [<<9>> EXCEPT ![1] = I(CHOOSE n \\in {@}: TRUE)][1] = 4\n====\n",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "8c3f641ad869c3c48d8ab370bd4d4f2251badb0856857a2d13aa1950c481ca59",
      Wasm: "8c3f641ad869c3c48d8ab370bd4d4f2251badb0856857a2d13aa1950c481ca59",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
  {
    id: "sharing-choose-old-predicate-argument-step3f-violation",
    input: {
      source:
        "---- MODULE Differential ----\nEXTENDS Integers, Sequences\nVARIABLE x\nI(p) == [<<4>> EXCEPT ![1] = p][1]\nInit == x = 0\nNext == UNCHANGED x\nSafe == [<<9>> EXCEPT ![1] = I(CHOOSE n \\in {9}: n = @)][1] = 4\n====\n",
      config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
    },
    digests: {
      TS: "03c4228876f45f3dc44da11459c2e4152a45a380fc55e6aad794ca1430d1d23b",
      native:
        "ef1f19686a386c809c007eb517dbcee34fae9291b20d603bdeeaa97c3430ba80",
      Wasm: "ef1f19686a386c809c007eb517dbcee34fae9291b20d603bdeeaa97c3430ba80",
    },
    outcomes: {
      TS: {
        kind: "error",
        code: "protocol",
        message: "Unsupported TLA input: expression Choose",
      },
      native: {
        kind: "compiled",
      },
      Wasm: {
        kind: "compiled",
      },
    },
  },
] as const;
const nestedOldActuals = new Set([
  "domain caller old actual",
  "predicate caller old actual",
  "unused domain caller old actual",
  "unused predicate caller old actual",
  "sharing-choose-old-domain-argument-step3f",
  "sharing-choose-old-predicate-argument-step3f",
  "sharing-choose-old-domain-argument-step3f-violation",
  "sharing-choose-old-predicate-argument-step3f-violation",
]);
for (const entry of newChooseV1)
  for (const frontend of ["TS", "native", "Wasm"] as const)
    test(`analysis parity controls v1 new CHOOSE bytes/refusal ${entry.id}: ${frontend}`, () => {
      for (const irVersion of [undefined, 1] as const) {
        const input = {
          source: entry.input.source,
          config: entry.input.config,
          ...(irVersion === undefined ? {} : { irVersion }),
        };
        const outcome = compile(input, frontend);
        assert.equal(
          createHash("sha256").update(JSON.stringify(outcome)).digest("hex"),
          nestedOldActuals.has(entry.id)
            ? "b32bd9eb3aecc232bb1bd739b0ae5c8979da2bb99d1c016d43791b4b69669428"
            : entry.digests[frontend],
        );
        assert.ok(isRecord(outcome));
        if (nestedOldActuals.has(entry.id))
          assert.deepEqual(outcome, {
            kind: "error",
            code: "protocol",
            message:
              "Unsupported TLA input: EXCEPT @ arguments across a nested EXCEPT require safety IR v2",
          });
        else assert.equal(outcome.kind, entry.outcomes[frontend].kind);
      }
    });

const reusedDigests: readonly (readonly [string, string])[] = [
  [
    "reports.json",
    "4872699bcc40c2de125a135c78cb45070380a18edfad4f260768e8445a5c7128",
  ],
  [
    "manifest.json",
    "34c2af10e0dd997c43dc23ea7d3189b430c9f73d28a1d3e6f10efbb865ae3a6f",
  ],
  [
    "choose-order/model/Differential.tla",
    "4ad9bd1f15564691df2dc3fe54844d6da445913899c73f923f05ab7d568bf40b",
  ],
  [
    "choose-order/model/Differential.cfg",
    "900bf8fdf1b2a1902cac0f9e4cc8e7f1ed1edc055698c1ef9e3aee1b0e375375",
  ],
  [
    "choose-order/model/tlc.log",
    "aea3f0a9cc71dd70b2291e7f48f509cff1823b97390fa74000263cf898f7396f",
  ],
  [
    "choose-equivalent/model/Differential.tla",
    "9b7f5e52908972350b64b2877a04db787aa40f5c0716eeed8ceb8381fa40c655",
  ],
  [
    "choose-equivalent/model/Differential.cfg",
    "900bf8fdf1b2a1902cac0f9e4cc8e7f1ed1edc055698c1ef9e3aee1b0e375375",
  ],
  [
    "choose-equivalent/model/tlc.log",
    "650b8922b36e77f8cf2da7ce6ab9fb652b24f482f5851c64eb94446fa00c000e",
  ],
  [
    "choose-tuple/model/Differential.tla",
    "ad6a77bcb7f6d1df31753b0897c82b04f3243a8232427917dbf1076d44d3e244",
  ],
  [
    "choose-tuple/model/Differential.cfg",
    "900bf8fdf1b2a1902cac0f9e4cc8e7f1ed1edc055698c1ef9e3aee1b0e375375",
  ],
  [
    "choose-tuple/model/tlc.log",
    "0d36de91fb5bd388fa1cf7dab9b48934a86afaf43716467a5719c0d60e0f669e",
  ],
  [
    "choose-nested/model/Differential.tla",
    "af74c5e107520625ce51ef2821a91909a9cffd6080a53d54c259c5a77d68e818",
  ],
  [
    "choose-nested/model/Differential.cfg",
    "900bf8fdf1b2a1902cac0f9e4cc8e7f1ed1edc055698c1ef9e3aee1b0e375375",
  ],
  [
    "choose-nested/model/tlc.log",
    "28e1547d94c944830d10b6d40366a2781ff3a1e41fabb8ee98a819ac2233677f",
  ],
  [
    "choose-capture/model/Differential.tla",
    "7afbf33040d385e9fdd044845c3f38721fdc6b39089117c5259de8f7a7ea8aec",
  ],
  [
    "choose-capture/model/Differential.cfg",
    "900bf8fdf1b2a1902cac0f9e4cc8e7f1ed1edc055698c1ef9e3aee1b0e375375",
  ],
  [
    "choose-capture/model/tlc.log",
    "fc86114c39eba065681f11b85d070a1c23bd7e96b0ebfd737dba1f74561a8c7d",
  ],
  [
    "choose-no-witness/model/Differential.tla",
    "f938221095c995451887b645f66a3e7e895931e2cab18849e75d7fc003e705aa",
  ],
  [
    "choose-no-witness/model/Differential.cfg",
    "ea9bc9a520aaabeae3f9c97592849bf442e3249c8910603fd102a9531e07f97f",
  ],
  [
    "choose-no-witness/model/tlc.log",
    "fc56e01cdaf18516443380a1d40921047633933d0d09eba132a0412796a9b023",
  ],
  [
    "choose-empty/model/Differential.tla",
    "243b5e192abab557a3d8260a491f076989e01433e25d0a8b4c74582b40a4cbac",
  ],
  [
    "choose-empty/model/Differential.cfg",
    "ea9bc9a520aaabeae3f9c97592849bf442e3249c8910603fd102a9531e07f97f",
  ],
  [
    "choose-empty/model/tlc.log",
    "be27167edcff505c99b9285cc9bcf1cfc0fa10abc52573308e3d9c6af5196696",
  ],
  [
    "choose-lazy/model/Differential.tla",
    "ae6a3b7517412e29fe38c41694c02219b622de9ff25cb196bb067a49d30e6f99",
  ],
  [
    "choose-lazy/model/Differential.cfg",
    "ea9bc9a520aaabeae3f9c97592849bf442e3249c8910603fd102a9531e07f97f",
  ],
  [
    "choose-lazy/model/tlc.log",
    "0a198b185b857d9ad1fb8de52bc89102c4090c8ae4637cd07642e88f071b9e98",
  ],
  [
    "choose-predicate-error/model/Differential.tla",
    "9c5fb157bb55ce5a697631c5ed13a75bd68bc5da7ee20ad45a588e075c28d7dd",
  ],
  [
    "choose-predicate-error/model/Differential.cfg",
    "900bf8fdf1b2a1902cac0f9e4cc8e7f1ed1edc055698c1ef9e3aee1b0e375375",
  ],
  [
    "choose-predicate-error/model/tlc.log",
    "0b05a4f1213b6060d71bb53b6f7362d7f435008fbb7022a7e0c278cfd8f5c62a",
  ],
  [
    "choose-tuple-error/model/Differential.tla",
    "7f131af4f848641f23e78bfab41104aa3b89cc6abf3fdda7619d00171d3bfad2",
  ],
  [
    "choose-tuple-error/model/Differential.cfg",
    "900bf8fdf1b2a1902cac0f9e4cc8e7f1ed1edc055698c1ef9e3aee1b0e375375",
  ],
  [
    "choose-tuple-error/model/tlc.log",
    "2d96156ffe32e987d9a8af695d4ea59e29a53ba2790c6064d552c13b2a306665",
  ],
];
const reusedNames = [
  "choose-order",
  "choose-equivalent",
  "choose-tuple",
  "choose-nested",
  "choose-capture",
  "choose-no-witness",
  "choose-empty",
  "choose-lazy",
  "choose-predicate-error",
  "choose-tuple-error",
] as const;
test("analysis parity controls reused CHOOSE goldens retain every pinned digest", async () => {
  for (const [path, digest] of reusedDigests)
    assert.equal(
      createHash("sha256")
        .update(await readFile(`${reuseRoot}/${path}`))
        .digest("hex"),
      digest,
      path,
    );
});
for (const name of reusedNames) {
  const verify = pairedChecker();
  for (const frontend of ["TS", "native", "Wasm"] as const)
    test(`analysis parity controls reused TLC ${name}: ${frontend}`, async () => {
      const reports: unknown = JSON.parse(
        await readFile(`${reuseRoot}/reports.json`, "utf8"),
      );
      assert.ok(Array.isArray(reports));
      const report = reports.find(
        (entry) =>
          isRecord(entry) && isRecord(entry.input) && entry.input.name === name,
      );
      assert.ok(isRecord(report) && isRecord(report.reference));
      const log = await readFile(`${reuseRoot}/${name}/model/tlc.log`, "utf8");
      const input = {
        source: await readFile(
          `${reuseRoot}/${name}/model/Differential.tla`,
          "utf8",
        ),
        config: await readFile(
          `${reuseRoot}/${name}/model/Differential.cfg`,
          "utf8",
        ),
        irVersion: 2 as const,
      };
      const program = programOf(compile(input, frontend));
      const result = verify(program, frontend);
      if (["choose-no-witness", "choose-empty"].includes(name)) {
        assert.equal(report.reference.status, "model-error");
        assert.equal(report.reference.exit, 75);
        assert.match(log, /no element|No element/i);
        assert.deepEqual(result, noWitness);
      } else if (
        name === "choose-predicate-error" ||
        name === "choose-tuple-error"
      ) {
        assert.equal(report.reference.status, "model-error");
        assert.equal(report.reference.exit, 75);
        assert.match(
          log,
          name === "choose-predicate-error"
            ? /expected a boolean value/
            : /CHOOSE/,
        );
        assert.ok(isRecord(result));
        assert.equal(result.kind, "error");
        assert.equal(result.code, "protocol");
        assert.equal(
          result.message,
          name === "choose-predicate-error"
            ? "Expected a boolean"
            : "Bound tuple pattern has the wrong length",
        );
      } else {
        assert.equal(report.reference.status, "complete");
        assert.equal(report.reference.exit, 0);
        assert.match(log, /Model checking completed. No error has been found/);
        assertComplete(result);
      }
    });
}

const tinyNames = [
  "sharing-case-condition-step3f",
  "sharing-notin-formal-step3f",
  "sharing-choose-domain-caller-step3f",
  "sharing-choose-predicate-caller-step3f",
  "sharing-choose-old-domain-argument-step3f",
  "sharing-choose-old-predicate-argument-step3f",
  "sharing-choose-domain-caller-step3f-violation",
  "sharing-choose-predicate-caller-step3f-violation",
  "sharing-choose-old-domain-argument-step3f-violation",
  "sharing-choose-old-predicate-argument-step3f-violation",
  "sharing-choose-domain-caller-step3f-state-violation",
  "sharing-choose-predicate-caller-step3f-state-violation",
  "sharing-choose-old-domain-argument-step3f-state-violation",
  "sharing-choose-old-predicate-argument-step3f-state-violation",
] as const;
for (const name of tinyNames) {
  const verify = pairedChecker();
  for (const frontend of ["TS", "native", "Wasm"] as const)
    test(`analysis parity controls tiny TLC ${name}: ${frontend}`, async () => {
      const expected: unknown = JSON.parse(
        await readFile(`${fixtureRoot}/expected-step3f.json`, "utf8"),
      );
      assert.ok(isRecord(expected) && Array.isArray(expected.cases));
      const entry = expected.cases.find(
        (entry) => isRecord(entry) && entry.name === name,
      );
      assert.ok(isRecord(entry));
      for (const [pathKey, digestKey] of [
        ["source", "sourceSha256"],
        ["config", "configSha256"],
        ["log", "logSha256"],
      ]) {
        assert.equal(
          createHash("sha256")
            .update(await readFile(`${fixtureRoot}/${entry[pathKey ?? ""]}`))
            .digest("hex"),
          entry[digestKey ?? ""],
          name,
        );
      }
      const input = {
        source: await readFile(`${fixtureRoot}/${entry.source}`, "utf8"),
        config: await readFile(`${fixtureRoot}/${entry.config}`, "utf8"),
        irVersion: 2 as const,
      };
      const program = programOf(compile(input, frontend));
      const result = verify(program, frontend);
      if (name.endsWith("-state-violation")) {
        assert.equal(entry.status, "invariant-violation");
        assert.equal(entry.exit, 12);
        assert.ok(isRecord(result));
        assert.equal(result.kind, "safety", JSON.stringify(result));
        assert.ok(isRecord(result.result));
        assert.equal(result.result.status, entry.status);
        assert.equal(result.result.reason, null);
        assert.equal(result.result.depth, 0);
        assert.deepEqual(result.result.violation, entry.violation);
        return;
      }
      if (name.endsWith("-violation")) {
        assert.equal(entry.status, "input-error");
        assert.equal(entry.exit, 151);
        assert.deepEqual(result, {
          kind: "error",
          code: "protocol",
          message: "Constant invariant or property is false",
        });
      } else {
        assert.equal(entry.status, "success");
        assert.equal(entry.exit, 0);
        assert.equal(entry.states, 1);
        assert.equal(entry.generated, 2);
        assertComplete(result);
      }
    });
}

const analysisControlManifest = [
  ...callActualCases.flatMap((entry) => [
    ...["TS", "native", "Wasm"].map((frontend) => ({
      id: `controls/call-actual/${entry.id}/${frontend}`,
      routes: entry.expected.kind === "error" || frontend === "Wasm" ? 1 : 3,
    })),
    ...(entry.expected.kind === "compiled"
      ? [
          {
            id: `controls/call-actual/${entry.id}/normalized-v2-ir`,
            routes: 7,
          },
        ]
      : []),
  ]),
  { id: "controls/call-actual/sweep-coverage", routes: 0 },
  ...known.flatMap((entry) =>
    ["TS", "native", "Wasm"].map((frontend) => ({
      id: `controls/${entry.id}/${frontend}`,
      routes: frontend === "Wasm" ? 1 : 3,
    })),
  ),
  ...chooseFreeOld.flatMap((_, index) =>
    [false, true].flatMap((unused) =>
      ["TS", "native", "Wasm"].map((frontend) => ({
        id: `controls/choose-free-old/${index}/${unused}/${frontend}`,
        routes: 3,
      })),
    ),
  ),
  ...["head", "explicit"].flatMap((form) => [
    ...[0, 1, 2, 3].map((index) => ({
      id: `controls/function-domain/free-old/${form}/${index}`,
      routes: 7,
    })),
    ...["target", "membership"].map((label) => ({
      id: `controls/function-domain/${form}/${label}`,
      routes: 7,
    })),
  ]),
  ...chooseContexts.flatMap((entry) =>
    ["TS", "native", "Wasm"].map((frontend) => ({
      id: `controls/choose/${entry.id}/${frontend}`,
      routes: frontend === "Wasm" ? 1 : 3,
    })),
  ),
  ...chooseShapes.flatMap((shape) =>
    ["domain", "predicate"].flatMap((position) =>
      chooseProbes.flatMap((probe) =>
        ["TS", "native", "Wasm"].map((frontend) => ({
          id: `controls/choose-probe/${shape.id}/${position}/${probe.id}/${frontend}`,
          routes: frontend === "Wasm" ? 1 : 3,
        })),
      ),
    ),
  ),
  ...chooseShapes.flatMap((shape) =>
    ["domain", "predicate"].flatMap((position) =>
      ["TS", "native", "Wasm"].map((frontend) => ({
        id: `controls/choose-free-old-shape/${shape.id}/${position}/${frontend}`,
        routes: 3,
      })),
    ),
  ),
  ...actions.map((entry) => ({ id: `controls/action/${entry.id}`, routes: 3 })),
  ...["p", "[]p"].map((formula) => ({
    id: `controls/property/${formula}`,
    routes: 7,
  })),
  ...["default", "1"].map((version) => ({
    id: `controls/v1-choose/${version}`,
    routes: 7,
  })),
  { id: "controls/choose-digests", routes: 0 },
  ...reusedNames.flatMap((name) =>
    ["TS", "native", "Wasm"].map((frontend) => ({
      id: `controls/reused/${name}/${frontend}`,
      routes: frontend === "Wasm" ? 1 : 3,
    })),
  ),
  ...newChooseV1.flatMap((entry) =>
    ["TS", "native", "Wasm"].map((frontend) => ({
      id: `controls/v1-new-choose/${entry.id}/${frontend}`,
      routes: 2,
    })),
  ),
  ...tinyNames.flatMap((name) =>
    ["TS", "native", "Wasm"].map((frontend) => ({
      id: `controls/tiny/${name}/${frontend}`,
      routes: frontend === "Wasm" ? 1 : 3,
    })),
  ),
  { id: "controls/supplemental-manifest", routes: 0 },
] as const;

test("analysis parity controls supplemental manifest has unique bounded route IDs", async () => {
  const fixedManifest: unknown = JSON.parse(
    await readFile(`${fixtureRoot}/analysis-controls-manifest.json`, "utf8"),
  );
  assert.deepEqual(
    analysisControlManifest,
    fixedManifest,
    "all fixed supplemental IDs and route counts",
  );
  assert.equal(
    new Set(analysisControlManifest.map((entry) => entry.id)).size,
    analysisControlManifest.length,
  );
  assert.ok(
    analysisControlManifest.every(
      (entry) => entry.routes >= 0 && entry.routes <= 7,
    ),
  );
});
