import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  copyFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { arch, cpus, platform } from "node:os";
import { join } from "node:path";
import { digest } from "../usable/provenance.js";

const directory = process.argv[2];
assert.ok(directory);
await mkdir(directory);
const save = (path: string, value: unknown) =>
  writeFile(join(directory, path), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
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
  "crates/rtla-compiler/benches/frontend.rs",
  "crates/rtla-compiler/tests/frontend-completion.json",
])
  .toString()
  .split("\0")
  .filter(Boolean)
  .sort();
const sources = await Promise.all(
  paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
);
await save("manifest.json", {
  plan: "0070-tla-coverage",
  sources,
  driverSha256: digest(
    await readFile("evals/experiments/tla-coverage/bench.ts"),
  ),
  toolchain: execFileSync("rustc", ["--version"]).toString().trim(),
  environment: { platform: platform(), arch: arch(), cpu: cpus()[0]?.model },
  scope:
    "Kernel costs for seven fixed cases, parse / compile v2 / prepared check; 20 samples, 500ms warmup, 1s measurement. No baseline speedup claim.",
});
const result = spawnSync(
  "cargo",
  [
    "bench",
    "--locked",
    "--manifest-path",
    "crates/rtla-compiler/Cargo.toml",
    "--bench",
    "frontend",
  ],
  { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
);
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
    else if (
      name.startsWith("frontend-") &&
      name.endsWith("/new/estimates.json")
    ) {
      const data = await readFile(join(path, entry.name));
      const benchmark: unknown = JSON.parse(data.toString());
      estimates.push({ name, estimates: benchmark });
      await copyFile(
        join(path, entry.name),
        join(directory, `${estimates.length}.json`),
      );
    }
  }
};
await collect("crates/rtla-compiler/target/criterion", "");
const unchanged = (
  await Promise.all(
    sources.map(
      async (source) => source.sha256 === digest(await readFile(source.path)),
    ),
  )
).every(Boolean);
await save("result.json", {
  status:
    result.status === 0 && unchanged && estimates.length === 21
      ? "complete"
      : "failed",
  exit: result.status,
  error: result.error?.message,
  unchanged,
  estimates,
});
assert.equal(result.status, 0);
assert.ok(unchanged);
assert.equal(estimates.length, 21);
