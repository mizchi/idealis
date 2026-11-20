import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { test } from "node:test";
import { createParserWasm } from "../../evals/experiments/rtla-parser/wasm.js";
import { readArchive } from "../../evals/experiments/tla-coverage/archive.js";
import {
  normalizeRtla,
  normalizeSany,
} from "../../evals/experiments/tla-coverage/normalize.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

interface AuditResult {
  readonly status: string;
  readonly compatibility: string;
  readonly referenceFailures: readonly { readonly name: string }[];
}
interface LabelReference {
  readonly records: readonly {
    readonly results: readonly {
      readonly version: string;
      readonly kind: string;
    }[];
  }[];
}

test("native/Wasm label junctions match both saved SANY references", async () => {
  const root = "evals/runs/0068-label-junction-reference";
  const saved: LabelReference = JSON.parse(
    await readFile(join(root, "result.json"), "utf8"),
  );
  const wasm = await createParserWasm(
    await readFile(
      "crates/rtla/target/wasm32-unknown-unknown/release/rtla.wasm",
    ),
  );
  assert.equal(saved.records.length, 7);
  for (const [index, record] of saved.records.entries()) {
    const source = await readFile(
      join(root, String(index), "Input.tla"),
      "utf8",
    );
    const request = { kind: "module", source };
    const native: unknown = JSON.parse(
      execFileSync("crates/rtla/target/release/rtla", [], {
        input: `${JSON.stringify(request)}\n`,
      }).toString(),
    );
    assert.deepEqual(wasm.request(request), native, source);
    assert.ok(isRecord(native));
    const actual =
      native.kind === "module"
        ? "accepted"
        : isRecord(native.error)
          ? native.error.kind
          : undefined;
    assert.deepEqual(
      record.results.map((result) => result.version),
      ["1.7.4", "1.8.0"],
    );
    for (const reference of record.results)
      assert.equal(actual, reference.kind, `${reference.version}: ${source}`);
  }
});

test("rtla replays the expanded corpus with exact native/Wasm ASTs and independent SANY structure", async () => {
  const archive = await readArchive("0070-syntax-audit-2.zip");
  const inputs: readonly { source: string; name: string }[] = JSON.parse(
    archive.text("inputs.json"),
  );
  assert.equal(inputs.length, 1558);
  const wasm = await createParserWasm(
    await readFile(
      "crates/rtla/target/wasm32-unknown-unknown/release/rtla.wasm",
    ),
  );
  let structural = 0;
  for (const syntaxVersion of ["1.7.4", "1.8.0"] as const) {
    const native: unknown[] = execFileSync(
      "crates/rtla/target/release/rtla",
      [],
      {
        input: `${inputs.map((input) => JSON.stringify({ kind: "module", source: input.source, syntaxVersion })).join("\n")}\n`,
        maxBuffer: 128 * 1024 * 1024,
      },
    )
      .toString()
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    assert.equal(native.length, inputs.length);
    for (const [index, input] of inputs.entries()) {
      const expected: unknown = JSON.parse(
        archive.text(`cases/${index}/rtla-${syntaxVersion}.json`),
      );
      const reference: unknown = JSON.parse(
        archive.text(`cases/${index}/sany-${syntaxVersion}-forward.json`),
      );
      assert.deepEqual(
        native[index],
        expected,
        `${syntaxVersion} ${input.name}`,
      );
      assert.deepEqual(
        wasm.request({ kind: "module", source: input.source, syntaxVersion }),
        expected,
        `${syntaxVersion} ${input.name}`,
      );
      assert.ok(isRecord(expected) && isRecord(reference));
      if (expected.kind === "module" && reference.kind === "accepted") {
        assert.deepEqual(
          normalizeRtla(expected.module),
          normalizeSany(reference.tree),
          `${syntaxVersion} ${input.name}`,
        );
        structural++;
      }
    }
  }
  assert.equal(structural, 2521);
});

test("stable prefix binding is retained and reference exceptions are not syntax matches", async () => {
  const wasm = await createParserWasm(
    await readFile(
      "crates/rtla/target/wasm32-unknown-unknown/release/rtla.wasm",
    ),
  );
  const expression = wasm.request({
    kind: "expression",
    source: "SUBSET 1..2",
  });
  assert.ok(isRecord(expression) && isRecord(expression.expression));
  assert.equal(expression.expression.operator, "..");
  assert.ok(
    Array.isArray(expression.expression.args) &&
      isRecord(expression.expression.args[0]),
  );
  assert.equal(expression.expression.args[0].operator, "SUBSET");
  const stable: AuditResult = JSON.parse(
    await readFile("evals/runs/0068-stable-fixed/result.json", "utf8"),
  );
  const preview: AuditResult = JSON.parse(
    await readFile("evals/runs/0068-preview-fixed/result.json", "utf8"),
  );
  assert.equal(stable.status, "partial-reference");
  assert.equal(stable.referenceFailures.length, 1);
  assert.equal(stable.referenceFailures[0]?.name, "Blank File Parse Error");
  assert.equal(preview.status, "complete");
  assert.equal(preview.compatibility, "partial");
  for (const root of ["0068-stable-fixed", "0068-preview-fixed"]) {
    const response: unknown = JSON.parse(
      await readFile(`evals/runs/${root}/cases/132/sany.json`, "utf8"),
    );
    assert.ok(isRecord(response));
    assert.equal(response.kind, "syntax");
  }
});
