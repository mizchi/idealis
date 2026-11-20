import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { after, test } from "node:test";
import {
  bounds,
  config,
  names,
} from "../../evals/experiments/quint-generated/cases.js";
import { decodeSafetyProgram } from "../../packages/idealis/src/safety-program.js";
import {
  checkSafety,
  decodeSafetyResult,
} from "../../packages/idealis/src/safety.js";
import {
  checkTlaSource,
  compileTlaSource,
} from "../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import { startCiNative, createCiWasm } from "../ci/tlc-checkers.js";

const directory = "fixtures/quint-generated/generated";
const core = await createCiWasm();
const native = startCiNative();
after(() => native.close());
const oracle: unknown = JSON.parse(
  await readFile("fixtures/quint-generated/tlc-oracle.json", "utf8"),
);
assert.ok(Array.isArray(oracle) && oracle.length === names.length * 2);

test("Quint output is preserved byte for byte with its source provenance", async () => {
  const manifest: unknown = JSON.parse(
    await readFile(`${directory}/manifest.json`, "utf8"),
  );
  assert.ok(isRecord(manifest) && Array.isArray(manifest.records));
  assert.equal(manifest.quint, "0.33.1");
  assert.equal(manifest.records.length, names.length);
  for (const record of manifest.records) {
    assert.ok(isRecord(record) && names.some((name) => name === record.name));
    const source = await readFile(`${directory}/${record.name}.tla`);
    assert.equal(
      createHash("sha256").update(source).digest("hex"),
      record.tlaSha256,
    );
    assert.deepEqual(
      source,
      await readFile(`${directory}/${record.name}.stdout`),
    );
    assert.deepEqual(
      await readFile(`${directory}/${record.name}.qnt`),
      await readFile(`fixtures/quint-generated/${record.name}.qnt`),
    );
    assert.equal(
      createHash("sha256")
        .update(await readFile(`${directory}/${record.name}.qnt`))
        .digest("hex"),
      record.quintSha256,
    );
    for (const property of ["safety", "broken"] as const)
      assert.equal(
        await readFile(`${directory}/${record.name}-${property}.cfg`, "utf8"),
        config(property),
      );
  }
});

test("Criterion corpus uses unchanged Quint output and TLC counts", async () => {
  const cases: unknown = JSON.parse(
    await readFile("crates/rtla-compiler/tests/quint-generated.json", "utf8"),
  );
  assert.ok(Array.isArray(cases));
  assert.deepEqual(
    cases.map((entry) => entry.name),
    [...names],
  );
  for (const entry of cases) {
    assert.ok(isRecord(entry));
    assert.equal(entry.quint, "0.33.1");
    assert.equal(entry.apalache, "0.63.0");
    assert.equal(
      entry.source,
      await readFile(`${directory}/${entry.name}.tla`, "utf8"),
    );
    assert.equal(entry.config, config("safety"));
    const reference: unknown = oracle.find(
      (row: unknown) =>
        isRecord(row) && row.name === entry.name && row.property === "safety",
    );
    assert.ok(isRecord(reference));
    assert.equal(entry.sourceSha256, reference.tlaSha256);
    assert.equal(entry.expectedStatus, reference.status);
    assert.equal(entry.expectedStates, reference.states);
    assert.equal(entry.expectedGenerated, reference.generated);
  }
});

for (const irVersion of [undefined, 1, 2] as const)
  for (const name of names)
    for (const property of ["safety", "broken"] as const)
      test(`${name}: generated TLA+ ${property} v${irVersion ?? "default"} agrees in native and Wasm`, async () => {
        const input = {
          source: await readFile(`${directory}/${name}.tla`, "utf8"),
          config: config(property),
          ...(irVersion === undefined ? {} : { irVersion }),
        };
        const program = compileTlaSource(core, input);
        assert.equal(program.version, irVersion ?? 1);
        const compiled = await native.request({
          version: 1,
          request: { kind: "compile-tla", ...input },
        });
        assert.ok(isRecord(compiled));
        assert.equal(compiled.kind, "compiled", JSON.stringify(compiled));
        assert.deepEqual(decodeSafetyProgram(compiled.program), program);
        const result = checkTlaSource(core, input, bounds);
        assert.deepEqual(checkSafety(core, program, bounds), result);
        if (irVersion === 1)
          assert.deepEqual(
            checkTlaSource(
              core,
              { source: input.source, config: input.config },
              bounds,
            ),
            result,
          );
        const reference: unknown = oracle.find(
          (entry: unknown) =>
            isRecord(entry) &&
            entry.name === name &&
            entry.property === property,
        );
        assert.ok(isRecord(reference));
        assert.equal(
          createHash("sha256").update(input.source).digest("hex"),
          reference.tlaSha256,
        );
        assert.equal(
          createHash("sha256").update(input.config).digest("hex"),
          reference.configSha256,
        );
        assert.equal(result.status, reference.status);
        if (reference.states !== null)
          assert.equal(result.states, reference.states);
        if (reference.generated !== null)
          assert.equal(result.generated, reference.generated);
        const response = await native.request({
          version: 1,
          request: { kind: "check-tla", ...input, bounds },
        });
        assert.ok(isRecord(response));
        assert.equal(response.kind, "safety", JSON.stringify(response));
        assert.deepEqual(decodeSafetyResult(response.result), result);
        const prepared = await native.request({
          version: 1,
          request: { kind: "check-safety", program, bounds },
        });
        assert.ok(isRecord(prepared));
        assert.equal(prepared.kind, "safety", JSON.stringify(prepared));
        assert.deepEqual(decodeSafetyResult(prepared.result), result);
        assert.equal(
          result.status,
          property === "safety" ? "complete" : "invariant-violation",
        );
        assert.ok(result.states > 0);
        assert.equal(result.reason, null);
        if (property === "broken") {
          assert.equal(result.violation?.name, "broken");
          assert.ok(result.violation && result.violation.trace.length > 0);
          if (["Counter", "Choice", "Match"].includes(name))
            assert.ok(result.violation.trace.length > 1);
        }
      });
