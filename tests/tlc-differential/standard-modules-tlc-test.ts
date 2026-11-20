import assert from "node:assert/strict";
import { copyFile, mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  runTlc,
  tlaValue,
} from "../../evals/experiments/collections-temporal/tlc.js";
import { classifyTlc } from "../../evals/experiments/tlc-differential/compare.js";
import { loadTlcStandardModules } from "../../evals/experiments/tlc-differential/standard-modules.js";

const java = process.env.TLC_JAVA;
const jar = process.env.TLC_JAR;
assert.ok(java && jar, "Use just tlc-diff-test <java> <jar>");
const probe = {
  source: `---- MODULE Probe ----
EXTENDS Sequences, Apalache
VARIABLE x
Op(a,b) == Head(<<>>)
Init == x=ApaFoldSeqLeft(Op,9,<<>>)
Next == UNCHANGED x
Safe == x=9
====`,
  config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
};

test("TLC uses the pinned library path including spaces and preserves local module precedence", async () => {
  const root = await mkdtemp(join(tmpdir(), "idealis-tlc-modules-"));
  try {
    const modules = await loadTlcStandardModules();
    const libraryPath = join(root, "pinned modules");
    await mkdir(libraryPath);
    await copyFile(
      join(modules.libraryPath, "Apalache.tla"),
      join(libraryPath, "Apalache.tla"),
    );
    const loaded = await loadTlcStandardModules(libraryPath);
    const result = await runTlc(
      java,
      jar,
      join(root, "empty-sequence"),
      probe,
      "Probe",
      loaded,
    );
    assert.equal(classifyTlc(result), "complete", result.log);
    assert.equal(result.states, 1);
    const local = await runTlc(
      java,
      jar,
      join(root, "local-module"),
      {
        ...probe,
        source: probe.source.replace("Safe == x=9", "Safe == x=99"),
        modules: {
          Apalache:
            "---- MODULE Apalache ----\nApaFoldSeqLeft(Op(_,_),v,seq) == 99\n====",
        },
      },
      "Probe",
      loaded,
    );
    assert.equal(classifyTlc(local), "complete", local.log);
    const missing = await runTlc(
      java,
      jar,
      join(root, "missing-module"),
      probe,
      "Probe",
    );
    assert.equal(classifyTlc(missing), "infrastructure-error", missing.log);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rendered signed 32-bit boundaries retain their values in nested trace data", async () => {
  const root = await mkdtemp(join(tmpdir(), "idealis-tlc-integer-"));
  try {
    const value = tlaValue({
      type: "list",
      value: [
        { type: "int", value: "-2147483648" },
        { type: "int", value: "2147483647" },
        {
          type: "set",
          value: [
            { type: "int", value: "-2147483648" },
            { type: "int", value: "2147483647" },
          ],
        },
        {
          type: "map",
          value: [
            [
              { type: "int", value: "0" },
              { type: "int", value: "-2147483648" },
            ],
            [
              { type: "int", value: "-2147483648" },
              { type: "bool", value: true },
            ],
          ],
        },
      ],
    });
    const input = {
      source: `---- MODULE Probe ----\nEXTENDS Integers\nVARIABLE x\nInit == x = ${value}\nNext == UNCHANGED x\nSafe == x[1]=(-2147483647 - 1) /\\ x[2]=2147483647 /\\ x[3]={-2147483647 - 1,2147483647} /\\ x[4][0]=(-2147483647 - 1) /\\ x[4][-2147483647 - 1]=TRUE\n====`,
      config: probe.config,
    };
    const result = await runTlc(
      java,
      jar,
      join(root, "boundaries"),
      input,
      "Probe",
    );
    assert.equal(classifyTlc(result), "complete", result.log);
    assert.equal(result.states, 1);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
