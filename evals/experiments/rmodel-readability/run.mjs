import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Jev, JevError } from "jev-lint/client";

const fixture = "fixtures/rmodel-readability/cases.json";
const rcheck = "crates/rcheck/target/release/rcheck";
const model = "jev-1.14.0";
const repetitions = 3;
const variants = ["action", "behavior"];
const letters = ["A", "B", "C", "D"];
const [command, dir] = process.argv.slice(2);
assert.ok(
  dir && ["run", "replay"].includes(command),
  "usage: run.mjs run|replay <directory>",
);

function candidate(source) {
  const updated = source.replace(
    /^action ([^\n]+) \{\n([\s\S]*?)^\}/gm,
    (_, signature, body) => {
      const guards = [];
      const assignments = [];
      for (const line of body.trimEnd().split("\n")) {
        if (/^\s+\w+' = /.test(line))
          assignments.push(line.replace("' =", "="));
        else guards.push(line);
      }
      assert.ok(guards.length && assignments.length, signature);
      return [
        `behavior ${signature} {`,
        "  guard {",
        ...guards.map((line) => `  ${line}`),
        "  }",
        "  update {",
        ...assignments.map((line) => `  ${line}`),
        "  }",
        "}",
      ].join("\n");
    },
  );
  assert.notEqual(source, updated);
  assert.doesNotMatch(updated, /^action /m);
  return updated;
}

function check(...args) {
  return JSON.parse(execFileSync(rcheck, args, { encoding: "utf8" }));
}

function write(file, data) {
  writeFileSync(join(dir, file), JSON.stringify(data, null, 2) + "\n", {
    flag: "wx",
  });
}

function schedule(cases) {
  const order = [];
  for (let trial = 0; trial < repetitions; trial += 1) {
    for (const [index, entry] of cases.entries()) {
      const pair =
        (trial * cases.length + index) % 2 ? [...variants].reverse() : variants;
      for (const variant of pair) order.push({ id: entry.id, variant, trial });
    }
  }
  return order;
}

function questionOptions(question, trial) {
  return question.options.map(
    (_, index) => question.options[(index + trial) % letters.length],
  );
}

function score(cases, runs) {
  const results = cases.map((entry) => ({
    id: entry.id,
    action: 0,
    behavior: 0,
  }));
  const order = schedule(cases);
  assert.deepEqual(
    runs.map(({ id, variant, trial }) => ({ id, variant, trial })),
    order,
  );
  for (const run of runs) {
    if (
      run.model !== model ||
      run.choices?.length !== 3 ||
      run.choices.some((choice) => !letters.includes(choice))
    )
      return { status: "unmeasured" };
    const entry = cases.find((c) => c.id === run.id);
    const result = results.find((c) => c.id === run.id);
    for (const [index, question] of entry.questions.entries()) {
      if (
        questionOptions(question, run.trial)[
          letters.indexOf(run.choices[index])
        ] === question.options[question.correct]
      ) {
        result[run.variant] += 1;
      }
    }
  }
  const total = results.reduce(
    (sum, row) => ({
      action: sum.action + row.action,
      behavior: sum.behavior + row.behavior,
    }),
    { action: 0, behavior: 0 },
  );
  const improved =
    total.behavior > total.action &&
    results.every((row) => row.behavior >= row.action);
  return {
    status: "measured",
    perVariant: (runs.length / 2) * 3,
    total,
    results,
    improved,
  };
}

if (command === "replay") {
  const cases = JSON.parse(readFileSync(join(dir, "cases.json"), "utf8"));
  const runs = JSON.parse(readFileSync(join(dir, "responses.json"), "utf8"));
  const recorded = JSON.parse(readFileSync(join(dir, "summary.json"), "utf8"));
  assert.deepEqual(score(cases, runs), recorded);
  process.stdout.write(JSON.stringify(recorded) + "\n");
  if (recorded.status !== "measured") process.exitCode = 1;
} else {
  const cases = JSON.parse(readFileSync(fixture, "utf8"));
  mkdirSync(dir);
  write("cases.json", cases);
  const sources = new Map();
  const semantics = [];
  for (const entry of cases) {
    assert.equal(entry.questions.length, 3);
    for (const question of entry.questions) {
      assert.equal(new Set(question.options).size, letters.length);
      assert.ok(question.options[question.correct] !== undefined, entry.id);
    }
    const action = readFileSync(entry.baseline, "utf8");
    const behavior = candidate(action);
    sources.set(entry.id, { action, behavior });
    const newPath = join(dir, `${entry.id}.behavior.model`);
    writeFileSync(newPath, behavior, { flag: "wx" });
    assert.deepEqual(
      check("generate", newPath, "ir"),
      check("generate", entry.baseline, "ir"),
    );
    const expected = check(entry.baseline, "--checker", "reference");
    assert.ok(
      expected.length &&
        expected.every(
          (report) =>
            report.answers.length === 1 &&
            report.answers[0].coverage.kind === "complete" &&
            ["holds", "violated", "reachable", "unreachable"].includes(
              report.answers[0].verdict,
            ),
        ),
    );
    assert.deepEqual(check(newPath, "--checker", "reference"), expected);
    semantics.push({ id: entry.id, checks: expected.length, irMatches: true });
  }
  write("semantics.json", semantics);
  const client = new Jev({
    model,
    retries: 0,
    rateLimitRetries: 0,
    timeoutMs: 30_000,
    budget: { maxRequests: 18, maxUsd: 0.05 },
  });
  const responses = [];
  for (const task of schedule(cases)) {
    const entry = cases.find((c) => c.id === task.id);
    const source = sources
      .get(task.id)
      [task.variant].replace(/^\s*(?:invariant|reachable|scope)\s.*$/gm, "")
      .trim();
    const questions = Object.fromEntries(
      entry.questions.map((question, index) => [
        `q${index}`,
        {
          type: "choice",
          instructions: { question: question.text },
          criteria: Object.fromEntries(
            questionOptions(question, task.trial).map((option, i) => [
              letters[i],
              option,
            ]),
          ),
        },
      ]),
    );
    const started = performance.now();
    try {
      const response = await client.ask(
        {
          instructions:
            "以下のモデルだけを読み、各質問に選択肢を一つ選んで答える。書かれた条件と更新に従うこと。",
          source,
        },
        questions,
      );
      const choices = Object.keys(questions).map((key) => {
        const answer = response.answers?.[key];
        return answer?.type === "choice" ? answer.choice : null;
      });
      responses.push({
        ...task,
        model: response.model ?? null,
        choices,
        ms: Math.round(performance.now() - started),
      });
    } catch (error) {
      responses.push({
        ...task,
        error: error instanceof JevError ? error.kind : "other",
        ms: Math.round(performance.now() - started),
      });
    }
  }
  write("responses.json", responses);
  const summary = score(cases, responses);
  write("summary.json", summary);
  process.stdout.write(JSON.stringify(summary) + "\n");
  if (summary.status !== "measured") process.exitCode = 1;
}
