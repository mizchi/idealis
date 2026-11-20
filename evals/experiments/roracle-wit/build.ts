import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  copyFile,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

await mkdir(".artifacts", { recursive: true });
const temporary = await mkdtemp(resolve(".artifacts/typed-oracle-build-"));
const generated = "evals/experiments/roracle-wit/generated";
await mkdir(generated, { recursive: true });
execFileSync(
  "cargo",
  [
    "build",
    "--release",
    "--locked",
    "--manifest-path",
    "evals/experiments/roracle-wit/rust/Cargo.toml",
    "--target",
    "wasm32-unknown-unknown",
    "--features",
    "component",
  ],
  { stdio: "inherit" },
);
execFileSync(
  "wasm-tools",
  [
    "component",
    "new",
    "evals/experiments/roracle-wit/rust/target/wasm32-unknown-unknown/release/roracle_typed_probe.wasm",
    "-o",
    resolve(temporary, "component.wasm"),
  ],
  { stdio: "inherit" },
);
execFileSync(
  "pnpm",
  [
    "--dir",
    "evals/experiments/roracle-wit/toolchain",
    "install",
    "--ignore-workspace",
    "--frozen-lockfile",
    "--ignore-scripts",
  ],
  { stdio: "inherit" },
);
const toolchain: {
  readonly transpileBytes: (
    input: Uint8Array,
    options: {
      readonly name: string;
      readonly instantiation: "sync";
      readonly nodejsCompat: boolean;
      readonly wasiShim: boolean;
    },
  ) => Promise<{
    readonly imports: readonly string[];
    readonly files: Record<string, Uint8Array>;
  }>;
} = await import(
  pathToFileURL(
    resolve(
      "evals/experiments/roracle-wit/toolchain/node_modules/@bytecodealliance/jco-transpile/dist/index.js",
    ),
  ).href
);
const result = await toolchain.transpileBytes(
  await readFile(resolve(temporary, "component.wasm")),
  {
    name: "oracle",
    instantiation: "sync",
    nodejsCompat: false,
    wasiShim: false,
  },
);
assert.deepEqual(
  result.imports,
  [],
  "The oracle component must not require host services",
);
for (const [name, content] of Object.entries(result.files)) {
  const path = resolve(generated, name);
  await mkdir(dirname(path), { recursive: true });
  if (name.endsWith(".d.ts")) {
    const formatted = execFileSync(
      "pnpm",
      ["exec", "vp", "fmt", "--stdin-filepath", path],
      { input: content, encoding: "utf8" },
    );
    if (process.argv.includes("--check"))
      assert.equal(
        await readFile(path, "utf8"),
        formatted,
        `Stale WIT declaration: ${name}`,
      );
    else await writeFile(path, formatted);
  } else await writeFile(path, content);
}
await copyFile(
  resolve(temporary, "component.wasm"),
  resolve(generated, "component.wasm"),
);
execFileSync(
  "pnpm",
  [
    "exec",
    "napi",
    "build",
    "--manifest-path",
    "evals/experiments/roracle-wit/rust/Cargo.toml",
    "--package-json-path",
    "evals/experiments/roracle-wit/package.json",
    "--output-dir",
    temporary,
    "--dts",
    "native.d.ts",
    "--features",
    "node",
    "--release",
    "--",
    "--locked",
  ],
  { stdio: "inherit" },
);
const nativeTypes = resolve(generated, "native.d.ts");
const formatted = execFileSync(
  "pnpm",
  ["exec", "vp", "fmt", "--stdin-filepath", nativeTypes],
  {
    input: await readFile(resolve(temporary, "native.d.ts")),
    encoding: "utf8",
  },
);
if (process.argv.includes("--check"))
  assert.equal(
    await readFile(nativeTypes, "utf8"),
    formatted,
    "Stale native declaration",
  );
else await writeFile(nativeTypes, formatted);
const names = (await readdir(temporary)).filter((name) =>
  name.endsWith(".node"),
);
assert.equal(names.length, 1);
const name = names[0];
assert.ok(name);
await copyFile(
  resolve(temporary, name),
  resolve(generated, "roracle-typed.node"),
);
