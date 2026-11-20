import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  copyFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { arch, cpus, loadavg, platform } from "node:os";
import { join, resolve } from "node:path";
import { zipSync } from "fflate";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { digest } from "../usable/provenance.js";

interface Build {
  readonly status: string;
  readonly binaries: readonly {
    readonly path: string;
    readonly sha256: string;
  }[];
}
const [mode, directory, snapshot] = process.argv.slice(2);
assert.ok(directory);
await mkdir(directory);
const save = (name: string, value: unknown) =>
  writeFile(join(directory, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
if (mode === "freeze") {
  const paths = execFileSync("git", [
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
    "--",
    "crates/rtla/src",
    "crates/rtlc/src",
    "crates/rtla-compiler/src",
    "crates/rtla-compiler/Cargo.toml",
    "crates/rtla-compiler/Cargo.lock",
    "crates/rtla/Cargo.toml",
    "crates/rtla/Cargo.lock",
    "crates/rtlc/Cargo.toml",
    "crates/rtlc/Cargo.lock",
    "crates/rtla-compiler/benches",
    "crates/rtlc/examples/counter.rs",
    "crates/rtlc/tests/coverage-bench.json",
    "crates/rtla-compiler/examples/semantics-profile.rs",
    "crates/rtla-compiler/tests/frontend-semantics-bench.json",
  ])
    .toString()
    .split("\0")
    .filter(Boolean)
    .sort();
  const sources = await Promise.all(
    paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  );
  const sourceFiles = Object.fromEntries(
    await Promise.all(paths.map(async (path) => [path, await readFile(path)])),
  );
  const archive = zipSync(sourceFiles, { level: 9 });
  await writeFile(join(directory, "source.zip"), archive, { flag: "wx" });
  const environment = {
    ...process.env,
    CARGO_PROFILE_RELEASE_DEBUG: "1",
    CARGO_PROFILE_BENCH_DEBUG: "1",
  };
  await save("manifest.json", {
    plan: "0072-frontend-semantics",
    sources,
    sourceZipSha256: digest(archive),
    driverSha256: digest(
      await readFile("evals/experiments/frontend-semantics/bench.ts"),
    ),
    toolchain: execFileSync("rustc", ["--version"]).toString().trim(),
    environment: {
      platform: platform(),
      arch: arch(),
      cpu: cpus()[0]?.model,
      debug: 1,
    },
    scope:
      "Same Criterion binary frozen before tuning. Compile / fresh check / prepared check are separate. No JVM, IPC or cold startup speedup claim.",
  });
  const binaries = [];
  for (const [name, args] of [
    ["benchmark", ["bench", "--no-run", "--bench", "frontend_semantics"]],
    [
      "semantics-profile",
      ["build", "--release", "--example", "semantics-profile"],
    ],
  ] as const) {
    const command = [
      ...args,
      "--locked",
      "--manifest-path",
      "crates/rtla-compiler/Cargo.toml",
      "--message-format=json",
    ];
    const result = spawnSync("cargo", command, {
      env: environment,
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    });
    await writeFile(
      join(directory, `${name}-build.log`),
      result.stdout + result.stderr,
      { flag: "wx" },
    );
    assert.equal(result.status, 0, result.stderr);
    const messages: unknown[] = result.stdout
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
    const entry = messages.find(
      (message) =>
        isRecord(message) &&
        typeof message.executable === "string" &&
        isRecord(message.target) &&
        message.target.name ===
          (name === "benchmark" ? "frontend_semantics" : "semantics-profile"),
    );
    assert.ok(isRecord(entry) && typeof entry.executable === "string");
    await copyFile(entry.executable, join(directory, name));
    binaries.push({
      path: name,
      sha256: digest(await readFile(join(directory, name))),
    });
  }
  for (const source of sources)
    assert.equal(
      digest(await readFile(source.path)),
      source.sha256,
      source.path,
    );
  await save("build.json", { status: "complete", binaries });
} else {
  assert.equal(mode, "run");
  assert.ok(snapshot);
  const manifest = await readFile(join(snapshot, "manifest.json"));
  const build: Build = JSON.parse(
    await readFile(join(snapshot, "build.json"), "utf8"),
  );
  assert.equal(build.status, "complete");
  const binary = build.binaries.find((binary) => binary.path === "benchmark");
  assert.ok(binary);
  assert.equal(
    digest(await readFile(join(snapshot, binary.path))),
    binary.sha256,
  );
  await save("manifest.json", {
    plan: "0072-frontend-semantics",
    snapshot,
    build,
    buildManifestSha256: digest(manifest),
    driverSha256: digest(
      await readFile("evals/experiments/frontend-semantics/bench.ts"),
    ),
    samples: 20,
    warmupMs: 500,
    measurementMs: 1000,
    load: loadavg(),
  });
  const result = spawnSync(resolve(snapshot, binary.path), ["--bench"], {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    env: { ...process.env, CRITERION_HOME: resolve(directory, "criterion") },
  });
  await writeFile(
    join(directory, "criterion.log"),
    result.stdout + result.stderr,
    { flag: "wx" },
  );
  const estimates: unknown[] = [];
  const collect = async (path: string, relative: string): Promise<void> => {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const name = relative ? `${relative}/${entry.name}` : entry.name;
      if (entry.isDirectory()) await collect(join(path, entry.name), name);
      else if (name.endsWith("/new/estimates.json"))
        estimates.push({
          name,
          estimates: JSON.parse(await readFile(join(path, entry.name), "utf8")),
        });
    }
  };
  await collect(join(directory, "criterion"), "");
  assert.equal(
    digest(await readFile(join(snapshot, binary.path))),
    binary.sha256,
  );
  await save("result.json", {
    status:
      result.status === 0 && estimates.length === 21 ? "complete" : "failed",
    exit: result.status,
    error: result.error?.message,
    estimates,
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(estimates.length, 21);
}
