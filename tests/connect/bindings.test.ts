import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";

function run(cmd: string, args: string[], cwd?: string) {
  const result = spawnSync(cmd, args, { cwd, encoding: "utf8" });
  if (result.error) throw result.error;
  return {
    status: result.status,
    output: result.stdout + result.stderr,
    stdout: result.stdout,
  };
}

test("Quint variants require handlers even when no trace exercises them", async () => {
  const dir = await mkdtemp(join(tmpdir(), "idealis-connect-types-"));
  try {
    const source = await readFile("fixtures/connect/TypedChoice.qnt", "utf8");
    const model = join(dir, "TypedChoice.qnt");
    await writeFile(model, source);
    function generate(template: string) {
      return run(process.execPath, [
        "scripts/connect-bindings.ts",
        model,
        "TypedChoice",
        template,
      ]);
    }
    async function bindings() {
      const custom = generate("fixtures/connect/templates/callbacks.mjs");
      assert.equal(custom.status, 0, custom.output);
      assert.ok(
        custom.stdout.startsWith("import type { Driver }"),
        "the template owns every output byte",
      );
      await writeFile(join(dir, "callbacks.ts"), custom.stdout);
      for (const lang of ["ts", "rs"]) {
        const result = generate(lang);
        assert.equal(result.status, 0, result.output);
        await writeFile(join(dir, "bindings." + lang), result.stdout);
      }
    }
    await bindings();
    for (const name of [
      "Cargo.toml",
      "store.rs",
      "store.ts",
      "callbacks-consumer.ts",
    ]) {
      await copyFile(join("fixtures/connect/consumer", name), join(dir, name));
    }
    const ts = () =>
      run(
        resolve("node_modules/.bin/tsc"),
        [
          "--ignoreConfig",
          "--target",
          "es2022",
          "--strict",
          "--module",
          "commonjs",
          "store.ts",
          "callbacks-consumer.ts",
        ],
        dir,
      );
    const rust = () => run("cargo", ["run", "--quiet"], dir);
    for (const check of [
      ts,
      () => run(process.execPath, ["store.js"], dir),
      () => run(process.execPath, ["callbacks-consumer.js"], dir),
      rust,
    ]) {
      const result = check();
      assert.equal(result.status, 0, result.output);
    }
    await writeFile(
      model,
      source
        .replace("Init | Put", "Init | Clear | Put")
        .replace(
          "| Init =>",
          "| Clear => observation.value == 0\n    | Init =>",
        ),
    );
    await bindings();
    const typescript = ts();
    for (const [result, missing] of [
      [typescript, /Clear/],
      [rust(), /handle_clear/],
    ] as const) {
      assert.notEqual(result.status, 0);
      assert.match(result.output, missing);
    }
    assert.match(typescript.output, /onClear/);
    await writeFile(
      model,
      source
        .replaceAll("value: int", "value: int, tags: Set[int]")
        .replaceAll("value: 0", "value: 0, tags: Set(0)")
        .replaceAll("value: selected", "value: selected, tags: Set(selected)"),
    );
    const unsupported = generate("ts");
    assert.notEqual(unsupported.status, 0);
    assert.match(unsupported.output, /Observation.tags.*set/i);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("templates receive source names and control their entire output", async () => {
  const dir = await mkdtemp(join(tmpdir(), "idealis-template-"));
  try {
    const source = (await readFile("fixtures/connect/TypedChoice.qnt", "utf8"))
      .replaceAll("Command", "Operation")
      .replaceAll("Observation", "View")
      .replaceAll("command", "lastOp")
      .replaceAll("observation", "currentView")
      .replaceAll("selected", "selectedValue");
    const model = join(dir, "renamed.qnt");
    await writeFile(model, source);
    const template = join(dir, "our format.mjs");
    await writeFile(
      template,
      `export async function render(c) {
      return [c.version, c.module, c.bindings.command, c.bindings.observation,
        c.variants[1].fields[0].name, c.observation[0].type].join("/");
    }`,
    );
    const args = [
      "scripts/connect-bindings.ts",
      model,
      "TypedChoice",
      template,
      "--command",
      "lastOp",
      "--observation",
      "currentView",
    ];
    const result = run(process.execPath, args);
    assert.equal(result.status, 0, result.output);
    assert.equal(
      result.stdout,
      "1/TypedChoice/lastOp/currentView/selectedValue/int",
    );
    await writeFile(template, "export function render() { return {}; }");
    const invalid = run(process.execPath, args);
    assert.notEqual(invalid.status, 0);
    assert.equal(invalid.stdout, "");
    assert.match(invalid.output, /must return a string/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
