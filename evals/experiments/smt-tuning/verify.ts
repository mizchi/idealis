import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import type { ScriptInput } from "../smt-scripts/corpus.js";
import { verify as verifyScripts } from "../smt-scripts/verify.js";
import {
  bindings,
  createNative,
  createZ3,
  maxWork,
  processClient,
} from "../smt-subset/clients.js";
import type { createWasm } from "../smt-subset/wasm.js";
import { decode, type Reply, type Request } from "../smt-subset/wire.js";

const fixtures = async (
  core: Awaited<ReturnType<typeof createWasm>>,
  onRecord: (record: unknown) => void,
) => {
  const native = createNative();
  const z3 = await createZ3();
  try {
    for (const name of ["repeat-64", "scopes-32", "ite-64", "integer-ite"]) {
      const source = await readFile(
        `crates/rusmt/benches/fixtures/${name}.smt2`,
        "utf8",
      );
      const request = {
        kind: name === "integer-ite" ? "solve" : "run-script",
        source,
        maxWork,
      } satisfies Request;
      const actual = core.request(request);
      const result = await native.request(request);
      onRecord({ fixture: name, request, native: result, wasm: actual });
      assert.deepEqual(result, actual);
      const answers =
        actual.kind === "solution"
          ? [actual.result]
          : actual.kind === "script-results"
            ? actual.results
            : [];
      assert.equal(answers.length, name === "integer-ite" ? 1 : 64);
      const statuses = answers.map((answer, index) => {
        const expected =
          name === "scopes-32" && index % 2 === 0 ? "unsat" : "sat";
        assert.equal(answer.status, expected);
        return expected;
      });
      const input = source + (name === "integer-ite" ? "(check-sat)\n" : "");
      const expected = answers.flatMap((answer) =>
        answer.status === "sat" ? ["sat", "sat"] : [answer.status],
      );
      let query = 0;
      const validated = input
        .trim()
        .split("\n")
        .flatMap((line) => {
          if (!line.startsWith("(check-sat")) return [line];
          const answer = answers[query++];
          assert.ok(answer);
          if (answer.status !== "sat") return [line];
          const integers =
            name === "repeat-64"
              ? Array.from({ length: 32 }, (_, index) => `x${index}`)
              : name === "scopes-32"
                ? ["x", "y", "z"]
                : ["x"];
          assert.deepEqual(Object.keys(answer.values).sort(), integers.sort());
          assert.deepEqual(
            Object.keys(answer.booleans),
            name === "ite-64" || name === "integer-ite" ? ["b"] : [],
          );
          return [line, `(push 1)${bindings(answer)}${line}(pop 1)`];
        })
        .join("\n");
      assert.equal(query, answers.length);
      const raw = await z3.script(input);
      const checkedModels = await z3.script(validated);
      onRecord({
        fixtureOracle: name,
        source,
        raw,
        validatedSource: validated,
        validatedOutput: checkedModels,
      });
      assert.deepEqual(raw.trim().split(/\s+/), statuses);
      assert.deepEqual(checkedModels.trim().split(/\s+/), expected);
    }
    return { fixtures: 4, fixtureQueries: 193, z3Version: z3.version };
  } finally {
    await Promise.all([native.close(), z3.close()]);
  }
};

export const verify = async (
  inputs: readonly ScriptInput[],
  core: Awaited<ReturnType<typeof createWasm>>,
  onRecord: (record: unknown) => void,
  baselinePath?: string,
  baselineCore?: Awaited<ReturnType<typeof createWasm>>,
) => {
  const baseline = baselinePath
    ? processClient(baselinePath, [], true)
    : undefined;
  const pending: { request: Request; reply: Reply }[] = [];
  const checked = {
    request: (request: Request): Reply => {
      const reply = core.request(request);
      if (baselineCore) {
        const previous = baselineCore.request(request);
        onRecord({ baselineWasm: request, actual: reply, previous });
        assert.deepEqual(reply, previous);
      }
      pending.push({ request, reply });
      return reply;
    },
  };
  try {
    const scripts = await verifyScripts(inputs, checked, onRecord);
    const benchmark = await fixtures(checked, onRecord);
    if (baseline)
      for (const item of pending) {
        const previous = decode(
          JSON.parse(
            await baseline.request(
              JSON.stringify({ version: 1, request: item.request }),
            ),
          ),
        );
        onRecord({
          baselineNative: item.request,
          actual: item.reply,
          previous,
        });
        assert.deepEqual(item.reply, previous);
      }
    return {
      ...scripts,
      ...benchmark,
      baselineRequests: baseline ? pending.length : null,
      exactStats: Boolean(baseline),
    };
  } finally {
    await baseline?.close();
  }
};
