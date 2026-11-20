import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { catalog } from "../../evals/experiments/workspace-oracle/catalog.js";
import { assertOracle } from "../../evals/experiments/workspace-oracle/node.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

test("all workspace tuning cases have accepted deterministic full replies", () => {
  for (const suite of catalog()) {
    const request = suite.cases
      .map((input) => JSON.stringify(input.request))
      .join("\n");
    const invoke = () =>
      execFileSync(suite.binary, [], {
        input: `${request}\n`,
        maxBuffer: 16 * 1024 * 1024,
      })
        .toString()
        .trim()
        .split("\n")
        .map((line): unknown => JSON.parse(line));
    const first = invoke();
    const second = invoke();
    assert.equal(first.length, suite.cases.length);
    assertOracle(second, first, suite.id);
    for (const [index, input] of suite.cases.entries())
      for (const required of input.requirements) {
        const actual = required.pointer
          .split("/")
          .slice(1)
          .reduce<unknown>(
            (value, key) => (isRecord(value) ? value[key] : undefined),
            first[index],
          );
        assertOracle(
          actual,
          required.value,
          `${suite.id}/${input.id}/${required.pointer}`,
        );
      }
  }
});

test("tampered binaries, sources and changed requests are rejected before measurement", async () => {
  const root = await mkdtemp(join(tmpdir(), "idealis-snapshot-test-"));
  const before = join(root, "nested", "before");
  const after = join(root, "after");
  try {
    execFileSync(
      process.execPath,
      ["dist/evals/experiments/workspace-oracle/tune.js", "snapshot", before],
      { stdio: "pipe" },
    );
    await cp(before, after, { recursive: true });
    const manifest = JSON.parse(
      await readFile(join(after, "manifest.json"), "utf8"),
    );
    const binary = join(after, manifest.binaries[0].id);
    const bytes = await readFile(binary);
    await writeFile(binary, "corrupted");
    const run = () =>
      spawnSync(
        process.execPath,
        [
          "dist/evals/experiments/workspace-oracle/tune.js",
          "run",
          before,
          after,
          join(root, "run"),
        ],
        { encoding: "utf8" },
      );
    const corrupted = run();
    assert.notEqual(corrupted.status, 0);
    assert.match(corrupted.stderr, /snapshot binary changed/);
    await writeFile(binary, bytes);
    const source = join(after, "source", manifest.sources[0].path);
    const original = await readFile(source);
    await writeFile(source, "changed source");
    const stale = run();
    assert.notEqual(stale.status, 0);
    assert.ok(stale.stderr.includes(manifest.sources[0].path));
    await writeFile(source, original);
    manifest.binaries[0].cases[0].request = { kind: "unexpected" };
    await writeFile(join(after, "manifest.json"), JSON.stringify(manifest));
    const changed = run();
    assert.notEqual(changed.status, 0);
    assert.match(changed.stderr, /baseline\/candidate suite identities/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
