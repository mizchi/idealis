import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { setImmediate } from "node:timers/promises";
import { promisify } from "node:util";
import {
  recordReplay,
  transitionScenario,
  type ObservationAdapter,
  type Scenario,
} from "../../examples/model-patterns/replay.js";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/autosave/autosave.ideal";
const spec = "examples/model-patterns/autosave/spec.json";
const ruleId = "AUTOSAVE-STALE-RESPONSE";
const tabs = ["Tab0", "Tab1"] as const;
const run = promisify(execFile);

type Command =
  | { readonly kind: "edit" }
  | { readonly kind: "start" | "finish"; readonly tab: (typeof tabs)[number] };

function autosave(acceptStale: boolean): ObservationAdapter<Command> {
  let revision = 0;
  let saved: number | null = null;
  const requests = new Map<string, number>();
  return {
    async apply(command) {
      switch (command.kind) {
        case "edit":
          revision++;
          return;
        case "start":
          requests.set(command.tab, revision);
          return;
        case "finish": {
          const sent = requests.get(command.tab);
          assert.notEqual(sent, undefined);
          requests.delete(command.tab);
          const response = await setImmediate(sent);
          assert.equal(typeof response, "number");
          if (acceptStale || response === revision) saved = response;
        }
      }
    },
    async observe() {
      const pending = Object.fromEntries(
        tabs.map((tab) => [tab, requests.get(tab) ?? null]),
      );
      return { revision, saved, pending };
    },
    project: (state) => state,
  };
}

async function check(args: string[], expectedExit = 0) {
  let stdout: string;
  try {
    ({ stdout } = await run(checker, args));
  } catch (error) {
    if (expectedExit === 0) throw error;
    assert.ok(error instanceof Error && "code" in error && "stdout" in error);
    assert.equal(error.code, expectedExit, error.message);
    assert.ok(typeof error.stdout === "string");
    return JSON.parse(error.stdout);
  }
  assert.equal(expectedExit, 0, "the checker succeeded unexpectedly");
  return JSON.parse(stdout);
}

test("an autosave rule leads to a stale response and its recorded state difference", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "idealis-autosave-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const forward = await check([
    "traceability",
    spec,
    "--rule",
    ruleId,
    "--document",
    "examples/model-patterns/autosave/README.md",
  ]);
  assert.equal(forward.rules[0].claims[0].evidence.verdict, "holds");
  assert.ok(Number.isInteger(forward.rules[0].document.line));

  const source = await readFile(model, "utf8");
  const fault = source.replace(
    "if { tab.pending = revision }",
    "if { some tab.pending }",
  );
  assert.notEqual(fault, source);
  await mkdir(dirname(join(root, model)), { recursive: true });
  await writeFile(join(root, model), fault);
  const reverse = await check(
    ["traceability", spec, "--check", model, "StaleKeepsSaved", "--root", root],
    1,
  );
  assert.equal(reverse.rules[0].id, ruleId);
  const evidence = reverse.rules[0].claims[0].evidence;
  assert.equal(evidence.verdict, "violated");
  const counterexample: Scenario<Command> = { ...evidence.trace, model };
  assert.deepEqual(
    counterexample.commands.map(({ kind }) => kind),
    ["start", "edit", "finish"],
  );

  const outOfOrder = await transitionScenario<Command>(
    checker,
    model,
    "finish",
    "before(tab.pending) != before(revision) and before(saved) = before(revision)",
  );
  assert.deepEqual(
    outOfOrder.commands.map(({ kind }) => kind),
    ["start", "edit", "start", "finish", "finish"],
  );

  for (const [scenario, expectedSaved] of [
    [counterexample, null],
    [outOfOrder, 1],
  ] as const) {
    for (const acceptStale of [false, true]) {
      const result = await recordReplay(
        checker,
        scenario,
        autosave(acceptStale),
      );
      const observationPath = join(root, "observations.json");
      await writeFile(
        observationPath,
        JSON.stringify({
          config: scenario.config,
          initial: scenario.initial,
          commands: scenario.commands,
          observations: result.recording.observations,
        }),
      );
      const itf = await run(checker, ["record-itf", model, observationPath]);
      const recording = join(root, "saved.itf.json");
      await writeFile(recording, itf.stdout);
      const replay = await check(
        ["replay-itf", model, recording],
        acceptStale ? 1 : 0,
      );
      if (acceptStale) {
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, scenario.commands.length);
        const pending = { Tab0: null, Tab1: null };
        assert.deepEqual(result.difference.expected, {
          revision: 1,
          saved: expectedSaved,
          pending,
        });
        assert.deepEqual(result.difference.observed, {
          revision: 1,
          saved: 0,
          pending,
        });
        assert.deepEqual(replay.difference, result.difference);
      } else {
        assert.equal(result.verdict, "matched");
        assert.deepEqual(replay, {
          verdict: "matched",
          steps: scenario.commands.length,
        });
      }
    }
  }
});
