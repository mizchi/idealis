import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { digest } from "../../evals/experiments/usable/provenance.js";
import type {
  SafetyResult,
  TlaSourceOptions,
} from "../../packages/idealis/src/contracts.js";
import { checkSafety } from "../../packages/idealis/src/safety.js";
import { compileTlaSafety } from "../../packages/idealis/src/tla-safety.js";
import {
  checkTlaSource,
  compileTlaSource,
} from "../../packages/idealis/src/tla-source.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import {
  capture,
  core,
  directory,
  native,
  normalize,
} from "./operator-sharing-support.js";

const bounds = { maxDepth: 64, maxStates: 10_000, maxWork: 1_000_000 };
interface Golden {
  readonly name: string;
  readonly source: string;
  readonly config: string;
  readonly log: string;
  readonly sourceSha256: string;
  readonly configSha256: string;
  readonly logSha256: string;
  readonly exit: number;
  readonly status: "success" | "invariant-violation" | "property-violation";
  readonly message?: string;
  readonly depth?: number;
  readonly states?: number;
  readonly generated?: number;
  readonly violation?: SafetyResult["violation"];
}
const golden: { readonly cases: readonly Golden[] } = JSON.parse(
  await readFile(`${directory}/expected-step3f-b20.json`, "utf8"),
);
const sources = new Map(
  await Promise.all(
    golden.cases.map(
      async (fixture) =>
        [
          fixture.name,
          {
            source: await readFile(`${directory}/${fixture.source}`, "utf8"),
            config: await readFile(`${directory}/${fixture.config}`, "utf8"),
          },
        ] as const,
    ),
  ),
);
const framedCases = golden.cases.filter((fixture) =>
  /^sharing-(un)?framed-/.test(fixture.name),
);
const versioned = (name: string, irVersion: 1 | 2): TlaSourceOptions => {
  const input = sources.get(name);
  assert.ok(input, name);
  return { ...input, irVersion };
};

const exitOf = {
  success: 0,
  "invariant-violation": 12,
  "property-violation": 13,
} as const;
const assertOracle = (fixture: Golden, outcome: unknown) => {
  assert.ok(isRecord(outcome), fixture.name);
  assert.equal(outcome.kind, "safety", JSON.stringify(outcome));
  assert.ok(isRecord(outcome.result));
  const result = outcome.result;
  if (fixture.status === "success") {
    assert.equal(result.status, "complete", JSON.stringify(result));
    assert.equal(result.states, fixture.states);
    assert.equal(result.generated, fixture.generated);
    assert.equal(result.violation, null);
    return;
  }
  assert.equal(result.status, fixture.status, JSON.stringify(result));
  assert.equal(result.reason, null);
  if (fixture.status === "invariant-violation") {
    assert.equal(result.depth, fixture.depth);
    assert.equal(result.states, fixture.states);
    assert.equal(result.generated, fixture.generated);
  }
  assert.deepEqual(result.violation, fixture.violation);
};

for (const fixture of golden.cases)
  test(`operator sharing b20 TLC fixture: ${fixture.name}`, async () => {
    const input = sources.get(fixture.name);
    assert.ok(input);
    const log = await readFile(`${directory}/${fixture.log}`, "utf8");
    assert.equal(digest(input.source), fixture.sourceSha256);
    assert.equal(digest(input.config), fixture.configSha256);
    assert.equal(digest(log), fixture.logSha256);
    assert.doesNotMatch(
      log,
      /\/Users\/|\/private\/|\/var\/folders|\[pid: \d+\]|\bon \d+ cores\b/,
    );
    assert.equal(fixture.exit, exitOf[fixture.status]);
    if (fixture.status === "success") {
      assert.match(log, /Model checking completed\. No error has been found\./);
    } else if (fixture.status === "invariant-violation") {
      assert.equal(fixture.message, "Invariant Safe is violated.");
      assert.ok(log.includes(`Error: ${fixture.message}\n`));
      assert.ok(fixture.violation);
      assert.equal(
        (log.match(/^State \d+:/gm) ?? []).length,
        fixture.violation.trace.length,
      );
      assert.match(log, /State 1: <Initial predicate>\nx = 0\n/);
      assert.match(log, /State 2: <Step [^\n]+>\nx = 1\n/);
      assert.ok(fixture.depth !== undefined);
      assert.ok(
        log.includes(
          `The depth of the complete state graph search is ${fixture.depth + 1}.`,
        ),
      );
    } else {
      assert.equal(
        fixture.message,
        "Property Safe is violated by the initial state:",
      );
      assert.ok(log.includes(`Error: ${fixture.message}\nx = 0\n`));
      assert.equal((log.match(/^State \d+:/gm) ?? []).length, 0);
    }
    if (fixture.states !== undefined)
      assert.ok(log.includes(`${fixture.states} distinct states found`));
    if (fixture.generated !== undefined)
      assert.ok(log.includes(`${fixture.generated} states generated`));
  });

for (const fixture of framedCases) {
  const constrained = /^CONSTRAINT /m.test(versioned(fixture.name, 2).config);
  for (const irVersion of [1, 2] as const) {
    const selected = versioned(fixture.name, irVersion);
    test(`operator sharing b20 v${irVersion} Rust frontend Wasm: ${fixture.name}`, () => {
      assertOracle(
        fixture,
        capture(() => ({
          kind: "safety",
          result: checkTlaSource(core, selected, bounds),
        })),
      );
    });
    test(`operator sharing b20 v${irVersion} native/Wasm check-tla parity: ${fixture.name}`, () => {
      const request = { kind: "check-tla" as const, ...selected, bounds };
      const wasm = capture(() => core.request({ version: 1, request }));
      const response = native(request);
      assert.deepEqual(response, wasm);
      assertOracle(fixture, response);
    });
    if (constrained) continue;
    test(`operator sharing b20 v${irVersion} TS frontend: ${fixture.name}`, () => {
      assertOracle(
        fixture,
        capture(() => ({
          kind: "safety",
          result: checkSafety(core, compileTlaSafety(selected), bounds),
        })),
      );
    });
    test(`operator sharing b20 v${irVersion} TS native/Wasm check-safety parity: ${fixture.name}`, () => {
      const request = {
        kind: "check-safety" as const,
        program: compileTlaSafety(selected),
        bounds,
      };
      const wasm = capture(() => core.request({ version: 1, request }));
      const response = native(request);
      assert.deepEqual(response, wasm);
      assertOracle(fixture, response);
    });
  }
  if (!constrained)
    test(`operator sharing b20 v2 frontend parity: ${fixture.name}`, () => {
      const selected = versioned(fixture.name, 2);
      const ts = capture(() => ({
        kind: "compiled",
        program: compileTlaSafety(selected),
      }));
      const rust = capture(() => ({
        kind: "compiled",
        program: compileTlaSource(core, selected),
      }));
      assert.deepEqual(normalize(ts), normalize(rust));
      assert.equal(ts.kind, "compiled", JSON.stringify(ts));
    });
}
