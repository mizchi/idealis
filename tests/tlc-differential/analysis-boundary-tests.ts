import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { parseModule } from "tla-ts";
import { createParserWasm } from "../../evals/experiments/rtla-parser/wasm.js";
import type { SafetyProgram } from "../../packages/idealis/src/contracts.js";
import { createPortableWasm } from "../../packages/idealis/src/portable-wasm.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

interface CompileExpectation {
  readonly kind: "compiled" | "error" | "syntax-error";
  readonly code?: string;
  readonly message?: string;
}
interface ParserExpectation {
  readonly kind?: string;
  readonly name?: string;
  readonly operator?: string;
  readonly args?: number;
  readonly operandUnary?: string;
  readonly error?: string;
  readonly errorKind?: "syntax" | "unsupported";
  readonly path?: readonly (string | number)[];
}
interface BoundaryCase {
  readonly id: string;
  readonly scope: string;
  readonly semanticReached: false;
  readonly source: string;
  readonly config: string;
  readonly irVersion: 2;
  readonly expression?: string;
  readonly tsAst?: ParserExpectation;
  readonly tsCompile: CompileExpectation;
  readonly rustParser?: ParserExpectation;
  readonly rustCompile?: CompileExpectation;
  readonly complete?: boolean;
  readonly equivalent?: string;
  readonly annotation?: string;
  readonly routes: readonly string[];
}
const directory = "fixtures/tlc-operator-sharing";
const manifest: {
  readonly schemaVersion: number;
  readonly sourceReach: false;
  readonly inputCap: number;
  readonly inputs: number;
  readonly routeCalls: number;
  readonly expectedIds: readonly string[];
  readonly cases: readonly BoundaryCase[];
} = JSON.parse(
  await readFile(`${directory}/analysis-boundary-manifest.json`, "utf8"),
);
const roles: readonly {
  readonly space: string;
  readonly constructor: string;
  readonly position: string;
  readonly role: string;
}[] = JSON.parse(await readFile(`${directory}/analysis-roles.json`, "utf8"));
const parser = await createParserWasm(
  await readFile("crates/rtla/target/wasm32-unknown-unknown/release/rtla.wasm"),
);
const core = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const bounds = { maxDepth: 8, maxStates: 100, maxWork: 10_000 };
const attempt = (run: () => unknown): unknown => {
  try {
    return run();
  } catch (error) {
    assert.ok(error instanceof Error);
    return {
      kind: "error",
      message: error.message,
      ...("code" in error ? { code: error.code } : {}),
    };
  }
};
const native = (binary: string, request: unknown): unknown => {
  const result = spawnSync(binary, [], {
    input: `${JSON.stringify(request)}\n`,
    encoding: "utf8",
    timeout: 1000,
    maxBuffer: 16 * 1024 * 1024,
  });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
};
const coreNative = (request: unknown) =>
  native("crates/idealis-core/target/release/idealis-core", {
    version: 1,
    request,
  });
const normalize = (value: unknown): unknown => {
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
const expectedCompile = (
  actual: unknown,
  expected: CompileExpectation,
  route: string,
) => {
  assert.ok(isRecord(actual), route);
  if (expected.kind === "compiled") {
    assert.equal(
      actual.kind,
      "compiled",
      `${route}: ${JSON.stringify(actual)}`,
    );
    assert.ok(isRecord(actual.program), route);
  } else {
    assert.equal(actual.kind, "error", `${route}: ${JSON.stringify(actual)}`);
    assert.equal(actual.message, expected.message, route);
    if (expected.kind !== "syntax-error")
      assert.equal(actual.code, expected.code, route);
  }
};
const atPath = (value: unknown, path: readonly (string | number)[] = []) =>
  path.reduce<unknown>((current, key) => {
    assert.ok(isRecord(current) || Array.isArray(current));
    return (current as Record<string | number, unknown>)[key];
  }, value);
const expectedAst = (
  actual: unknown,
  expected: ParserExpectation,
  route: string,
) => {
  assert.ok(isRecord(actual), route);
  const node = atPath(actual, expected.path);
  assert.ok(isRecord(node), route);
  if (expected.kind) assert.equal(node.kind, expected.kind, route);
  if (expected.name) assert.equal(node.name, expected.name, route);
  if (expected.operator) assert.equal(node.operator, expected.operator, route);
  if (expected.args !== undefined) {
    assert.ok(Array.isArray(node.args), route);
    assert.equal(node.args.length, expected.args, route);
  }
  if (expected.operandUnary) {
    assert.ok(Array.isArray(node.args));
    assert.ok(isRecord(node.args[1]));
    assert.equal(node.args[1].operator, expected.operandUnary, route);
  }
};
const complete = (program: SafetyProgram) => {
  const request = { kind: "check-safety" as const, program, bounds };
  const wasm = attempt(() => core.request({ version: 1, request }));
  assert.deepEqual(coreNative(request), wasm);
  assert.ok(isRecord(wasm) && wasm.kind === "safety");
  assert.ok(isRecord(wasm.result));
  assert.equal(wasm.result.status, "complete");
  assert.equal(wasm.result.reason, null);
  assert.equal(wasm.result.states, 1);
  assert.equal(wasm.result.generated, 2);
  assert.equal(wasm.result.depth, 0);
  assert.equal(wasm.result.violation, null);
  return wasm;
};

test("analysis parity boundaries fixed ID/routes and supplement bound", () => {
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.sourceReach, false);
  assert.equal(manifest.inputCap, 256);
  assert.equal(manifest.inputs, manifest.cases.length);
  assert.ok(manifest.inputs <= manifest.inputCap);
  assert.equal(new Set(manifest.expectedIds).size, manifest.inputs);
  assert.deepEqual(
    manifest.cases.map((entry) => entry.id),
    manifest.expectedIds,
  );
  assert.equal(
    manifest.routeCalls,
    manifest.cases.reduce((count, entry) => count + entry.routes.length, 0),
  );
  for (const entry of manifest.cases) {
    assert.equal(entry.semanticReached, false);
    assert.ok(Buffer.byteLength(entry.source) <= 2048, entry.id);
    assert.equal(new Set(entry.routes).size, entry.routes.length, entry.id);
    assert.deepEqual(
      entry.routes,
      [
        "TS-compile",
        ...(entry.tsAst ? ["TS-parser"] : []),
        ...(entry.rustParser ? ["Rust-native-parser", "Rust-Wasm-parser"] : []),
        ...(entry.rustCompile
          ? ["Rust-native-compile", "Rust-Wasm-compile"]
          : []),
        ...(entry.complete
          ? [
              "TS-IR-native-check",
              "TS-IR-Wasm-check",
              "Rust-IR-native-check",
              "Rust-IR-Wasm-check",
            ]
          : []),
        ...(entry.equivalent
          ? [`TS-equivalence-compile:${entry.equivalent}`]
          : []),
        ...(entry.annotation ? ["TS-annotation-parser"] : []),
        ...(entry.id === "adapter/except-field"
          ? ["TS-field-adapter-parser"]
          : []),
      ],
      entry.id,
    );
  }
});

for (const entry of manifest.cases)
  test(`analysis parity boundaries ${entry.id}`, () => {
    if (entry.tsAst) {
      const parsed = attempt(() => ({
        kind: "parsed",
        module: parseModule(entry.source),
      }));
      assert.ok(isRecord(parsed));
      if (entry.tsAst.error) {
        assert.equal(parsed.kind, "error");
        assert.equal(parsed.message, entry.tsAst.error);
      } else {
        assert.equal(parsed.kind, "parsed", JSON.stringify(parsed));
        assert.ok(
          isRecord(parsed.module) && Array.isArray(parsed.module.definitions),
        );
        const probe = parsed.module.definitions.find(
          (definition: unknown) =>
            isRecord(definition) && definition.name === "Probe",
        );
        assert.ok(isRecord(probe));
        expectedAst(probe.body, entry.tsAst, `${entry.id}/TS-parser`);
      }
    }
    if (entry.rustParser) {
      assert.equal(typeof entry.expression, "string");
      const request = { kind: "expression", source: entry.expression };
      const rust = native("crates/rtla/target/release/rtla", request);
      assert.deepEqual(
        parser.request(request),
        rust,
        `${entry.id}/Rust parser native/Wasm`,
      );
      assert.ok(isRecord(rust));
      if (entry.rustParser.error) {
        assert.equal(rust.kind, "error");
        assert.ok(isRecord(rust.error));
        assert.equal(rust.error.kind, entry.rustParser.errorKind ?? "syntax");
        assert.equal(rust.error.message, entry.rustParser.error);
      } else {
        assert.equal(rust.kind, "expression");
        expectedAst(
          rust.expression,
          entry.rustParser,
          `${entry.id}/Rust-parser`,
        );
      }
    }
    const input = {
      source: entry.source,
      config: entry.config,
      irVersion: entry.irVersion,
    };
    const ts = attempt(() => ({
      kind: "compiled",
      program: compileTlaSafety(input),
    }));
    expectedCompile(ts, entry.tsCompile, `${entry.id}/TS-compile`);
    if (entry.rustCompile) {
      const request = { kind: "compile-tla" as const, ...input };
      const rust = coreNative(request);
      assert.deepEqual(
        attempt(() => core.request({ version: 1, request })),
        rust,
        `${entry.id}/Rust compile native/Wasm`,
      );
      expectedCompile(rust, entry.rustCompile, `${entry.id}/Rust-compile`);
      if (
        isRecord(ts) &&
        isRecord(rust) &&
        ts.kind === "compiled" &&
        rust.kind === "compiled"
      ) {
        assert.deepEqual(normalize(ts.program), normalize(rust.program));
        if (entry.complete)
          assert.deepEqual(
            complete(ts.program as SafetyProgram),
            complete(rust.program as SafetyProgram),
          );
      }
    }
    if (entry.equivalent) {
      const counterpart = manifest.cases.find(
        (candidate) => candidate.id === entry.equivalent,
      );
      assert.ok(counterpart);
      const program = compileTlaSafety({
        source: counterpart.source,
        config: counterpart.config,
        irVersion: 2,
      });
      assert.ok(isRecord(ts));
      assert.deepEqual(normalize(program), normalize(ts.program));
    }
    if (entry.annotation) {
      const module = parseModule(entry.source);
      const body = module.definitions.find(
        (definition) => definition.name === "Probe",
      )?.body;
      assert.ok(body && body.kind === "WeakFairness");
      assert.equal(body.enabled, undefined);
      const annotated = { ...body, enabled: body.action };
      assert.deepEqual(annotated.enabled, body.action);
      assert.equal(Object.hasOwn(body, "enabled"), false);
      assert.ok(
        roles.some(
          (row) =>
            row.space === "parser-only" &&
            row.constructor === "Fairness" &&
            row.position === "enabled" &&
            row.role === "annotation",
        ),
      );
    }
    if (entry.id === "adapter/except-field") {
      const body = parseModule(entry.source).definitions.find(
        (definition) => definition.name === "Probe",
      )?.body;
      assert.ok(body && body.kind === "Apply");
      const access = body.args[0];
      assert.ok(access && access.kind === "FieldAccess");
      const update = access.record;
      assert.equal(update.kind, "Except");
      if (update.kind === "Except") {
        assert.deepEqual(update.updates[0]?.path[0], {
          kind: "Field",
          name: "a",
        });
        assert.ok(
          roles.some(
            (row) =>
              row.space === "source-wrapper" &&
              row.constructor === "ExceptPath.Field" &&
              row.position === "",
          ),
        );
      }
    }
  });
