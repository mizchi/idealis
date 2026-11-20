import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { test } from "node:test";
import { bounds } from "../../evals/experiments/collections-temporal/cases.js";
import { createParserWasm } from "../../evals/experiments/rtla-parser/wasm.js";
import {
  cases,
  source,
} from "../../evals/experiments/symbolic-operators/corpus.js";
import { models } from "../../evals/experiments/symbolic-operators/models.js";
import type { Report } from "../../evals/experiments/tlc-differential/differential.js";
import { decodeSafetyResult } from "../../packages/idealis/src/safety.js";
import {
  checkTlaSource,
  compileTlaSource,
} from "../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import { startCiNative, createCiWasm } from "../ci/tlc-checkers.js";

interface SyntaxRun {
  readonly status: string;
  readonly records: readonly {
    readonly id: string;
    readonly native: unknown;
    readonly references: readonly {
      readonly version: string;
      readonly kind: string;
    }[];
  }[];
}
interface ModelRun {
  readonly status: string;
  readonly models: number;
  readonly records: readonly {
    readonly name: string;
    readonly kind: string;
    readonly irVersion?: 1 | 2;
    readonly report?: Report;
    readonly classification?: string;
    readonly actual?: readonly {
      readonly irVersion: 1 | 2;
      readonly response: unknown;
    }[];
  }[];
}

test("symbolic native/Wasm syntax and standalone fixtures retain both SANY controls", async () => {
  const root = "evals/runs/0069-parser";
  const saved: SyntaxRun = JSON.parse(
    await readFile(join(root, "result.json"), "utf8"),
  );
  assert.equal(saved.status, "complete");
  assert.equal(saved.records.length, 33);
  const fixture: unknown = JSON.parse(
    await readFile("crates/rtla/tests/fixtures/symbolic.json", "utf8"),
  );
  assert.deepEqual(
    fixture,
    cases.map((input) => ({
      id: input.id,
      source: source(input.body),
      accepted: input.accepted,
    })),
  );
  const wasm = await createParserWasm(
    await readFile(
      "crates/rtla/target/wasm32-unknown-unknown/release/rtla.wasm",
    ),
  );
  // Run 0069 predates the lexer recognizing (+) and (\X) as single tokens.
  const aliasExpectations: Record<string, unknown> = JSON.parse(
    await readFile(
      "fixtures/symbolic-operators/parenthesized-aliases.json",
      "utf8",
    ),
  );
  for (const [index, record] of saved.records.entries()) {
    const input = cases[index];
    assert.ok(input && input.id === record.id);
    const request = { kind: "module", source: source(input.body) };
    const native: unknown = JSON.parse(
      execFileSync("crates/rtla/target/release/rtla", [], {
        input: `${JSON.stringify(request)}\n`,
      }).toString(),
    );
    assert.deepEqual(
      native,
      aliasExpectations[record.id] ?? record.native,
      record.id,
    );
    assert.ok(isRecord(native));
    const syntax =
      native.kind === "module"
        ? "accepted"
        : isRecord(native.error) && native.error.kind;
    assert.equal(syntax, input.accepted ? "accepted" : "syntax", record.id);
    assert.deepEqual(wasm.request(request), native, record.id);
    assert.deepEqual(
      record.references.map((reference) => reference.version),
      ["1.7.4", "1.8.0"],
    );
    assert.ok(
      record.references.every((reference) => reference.kind === syntax),
      record.id,
    );
  }
});

test("IR v1/v2 replay TLC models, witnesses and symbolic conflict refusals", async () => {
  const root = "evals/runs/0069-models-final";
  const saved: ModelRun = JSON.parse(
    await readFile(join(root, "result.json"), "utf8"),
  );
  assert.equal(saved.status, "complete");
  assert.equal(saved.models, 22);
  assert.equal(saved.records.length, 34);
  const manifest: { readonly inputs: readonly unknown[] } = JSON.parse(
    await readFile(join(root, "manifest.json"), "utf8"),
  );
  assert.deepEqual(manifest.inputs, models);
  const core = await createCiWasm();
  const native = startCiNative();
  try {
    for (const record of saved.records) {
      const model = models.find((model) => model.name === record.name);
      assert.ok(model);
      if (record.report) {
        assert.ok(record.irVersion);
        const input = { ...model, irVersion: record.irVersion };
        const actual = checkTlaSource(core, input, bounds);
        assert.deepEqual(actual, record.report.ours.result, record.name);
        assert.deepEqual(record.report.differences, [], record.name);
        const response = await native.request({
          version: 1,
          request: {
            kind: "check-tla",
            source: input.source,
            config: input.config,
            irVersion: input.irVersion,
            bounds,
          },
        });
        assert.ok(isRecord(response) && response.kind === "safety");
        assert.deepEqual(
          decodeSafetyResult(response.result),
          actual,
          record.name,
        );
      } else {
        assert.equal(record.classification, "input-error", record.name);
        assert.equal(record.actual?.length, 2);
        for (const savedResponse of record.actual ?? []) {
          const input = { ...model, irVersion: savedResponse.irVersion };
          assert.throws(
            () => compileTlaSource(core, input),
            /builtin or imported symbol/,
          );
          const response = await native.request({
            version: 1,
            request: {
              kind: "compile-tla",
              source: input.source,
              config: input.config,
              irVersion: input.irVersion,
            },
          });
          assert.deepEqual(response, savedResponse.response, record.name);
        }
      }
    }
  } finally {
    await native.close();
  }
});
