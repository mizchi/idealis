import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { cases } from "../single-model/cases.js";

const directory = process.argv[2];
const java = process.argv[3];
const jar = process.argv[4];
assert.ok(directory && java && jar);
await mkdir(directory);
const inputs = await Promise.all(
  ["linear-ring", "function-cube"].map(async (name) => {
    const original = cases("small").find((input) => input.name === name);
    assert.ok(original);
    const input = {
      ...original,
      config: original.config.replace(
        /N=\d+/,
        `N=${name === "linear-ring" ? 1000 : 8}`,
      ),
    };
    const module = /MODULE (\w+)/.exec(input.source)?.[1];
    assert.ok(module);
    const path = resolve(directory, name);
    await mkdir(path);
    await writeFile(resolve(path, `${module}.tla`), input.source, {
      flag: "wx",
    });
    await writeFile(resolve(path, `${module}.cfg`), input.config, {
      flag: "wx",
    });
    await writeFile(resolve(path, "input.json"), JSON.stringify(input), {
      flag: "wx",
    });
    return {
      name,
      source: input.source,
      config: input.config,
      module,
      directory: path,
    };
  }),
);
const binary =
  "evals/experiments/oracle-comparison/rust/target/release/oracle-comparison";
const paths = [
  java,
  jar,
  process.execPath,
  binary,
  "crates/idealis-core/target/release/idealis-core",
  "dist/evals/experiments/oracle-comparison/worker.js",
  "node_modules/tla-ts/package.json",
];
const sources = execFileSync("git", [
  "ls-files",
  "--cached",
  "--others",
  "--exclude-standard",
  "crates/roracle",
  "crates/rtla",
  "crates/rtla-compiler",
  "crates/rtlc",
  "crates/idealis-core",
  "evals/experiments/oracle-comparison",
  "tests/oracle-comparison",
  "evals/plans/0051-oracle-library.md",
  "pnpm-lock.yaml",
])
  .toString()
  .trim()
  .split("\n");
const fingerprints = await Promise.all(
  [...new Set([...paths, ...sources])].map(async (path) => ({
    path,
    sha256: createHash("sha256")
      .update(await readFile(path))
      .digest("hex"),
  })),
);
const input = {
  inputs,
  java: resolve(java),
  jar: resolve(jar),
  node: process.execPath,
  native: resolve("crates/idealis-core/target/release/idealis-core"),
  worker: resolve("dist/evals/experiments/oracle-comparison/worker.js"),
};
await writeFile(resolve(directory, "input.json"), JSON.stringify(input), {
  flag: "wx",
});
await writeFile(
  resolve(directory, "manifest.json"),
  JSON.stringify({
    plan: "0051",
    head: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    node: process.version,
    rust: execFileSync("rustc", ["--version"]).toString().trim(),
    java: spawnSync(java, ["-version"], { encoding: "utf8" }).stderr.trim(),
    tlcRevision: "5a47802",
    fingerprints,
    boundaries: {
      cold: "includes process startup, transport, parsing and raw-log writes",
      prepared: "check plus serde JSON projection; excludes compile",
    },
  }),
  { flag: "wx" },
);
execFileSync(resolve(binary), [resolve(directory)], { stdio: "inherit" });

const evidence = await Promise.all(
  (await readdir(directory))
    .filter((name) => name.endsWith(".json"))
    .map(async (name) => ({
      path: name,
      sha256: createHash("sha256")
        .update(await readFile(resolve(directory, name)))
        .digest("hex"),
    })),
);
await writeFile(resolve(directory, "evidence.json"), JSON.stringify(evidence), {
  flag: "wx",
});
