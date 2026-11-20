import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { unzipSync, zipSync } from "fflate";
import { isRecord } from "../../../packages/idealis/src/trace.js";
import { digest } from "../usable/provenance.js";

interface Manifest {
  readonly sources: readonly {
    readonly path: string;
    readonly sha256: string;
  }[];
  readonly sourceZipSha256: string;
  readonly toolchain: string;
}
const [snapshot, directory] = process.argv.slice(2);
assert.ok(snapshot && directory);
await mkdir(directory);
const save = (name: string, value: unknown) =>
  writeFile(join(directory, name), `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
  });
const manifest: Manifest = JSON.parse(
  await readFile(join(snapshot, "manifest.json"), "utf8"),
);
const archive = await readFile(join(snapshot, "source.zip"));
assert.equal(digest(archive), manifest.sourceZipSha256);
assert.equal(
  execFileSync("rustc", ["--version"]).toString().trim(),
  manifest.toolchain,
);
const files = unzipSync(archive);
for (const source of manifest.sources) {
  const contents = files[source.path];
  assert.ok(contents, source.path);
  assert.equal(digest(contents), source.sha256, source.path);
}
const supplement: Record<string, Uint8Array> = {};
for (const path of [
  "crates/rtla/Cargo.toml",
  "crates/rtla/Cargo.lock",
  "crates/rtla-compiler/benches/compatibility.rs",
  "crates/rtla-compiler/benches/frontend.rs",
  "crates/rtlc/examples/counter.rs",
  "crates/rtlc/tests/coverage-bench.json",
]) {
  if (files[path]) continue;
  const frozen = execFileSync("git", ["show", `23131b74:${path}`]);
  assert.equal(digest(await readFile(path)), digest(frozen), path);
  supplement[path] = frozen;
}
const supplementArchive = zipSync(supplement, { level: 9 });
await writeFile(join(directory, "supplement.zip"), supplementArchive, {
  flag: "wx",
});
await save("manifest.json", {
  plan: "0071-recursive-actions",
  snapshot,
  snapshotManifestSha256: digest(
    await readFile(join(snapshot, "manifest.json")),
  ),
  sourceZipSha256: digest(archive),
  supplementZipSha256: digest(supplementArchive),
  supplementRevision: "23131b74",
  supplement: Object.entries(supplement).map(([path, contents]) => ({
    path,
    sha256: digest(contents),
  })),
  driverSha256: digest(
    await readFile("evals/experiments/recursive-actions/rebuild.ts"),
  ),
  scope:
    "Original snapshots omitted unchanged rtla manifests and unrelated Cargo target files. Add a verified supplement without editing original evidence. Rebuild from this closed source tree; binary identity across paths is not required.",
});
const root = resolve(directory, "source");
for (const [path, contents] of Object.entries({ ...files, ...supplement })) {
  assert.ok(path.startsWith("crates/") && !path.split("/").includes(".."));
  await mkdir(dirname(join(root, path)), { recursive: true });
  await writeFile(join(root, path), contents, { flag: "wx" });
}
const binaries: { name: string; path: string; sha256: string }[] = [];
for (const [name, args] of [
  ["recursive_actions", ["bench", "--no-run", "--bench", "recursive_actions"]],
  [
    "recursive-profile",
    ["build", "--release", "--example", "recursive-profile"],
  ],
] as const) {
  const result = spawnSync(
    "cargo",
    [
      ...args,
      "--locked",
      "--offline",
      "--manifest-path",
      "crates/rtla-compiler/Cargo.toml",
      "--message-format=json",
    ],
    {
      cwd: root,
      env: {
        ...process.env,
        CARGO_PROFILE_RELEASE_DEBUG: "1",
        CARGO_PROFILE_BENCH_DEBUG: "1",
      },
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    },
  );
  await writeFile(
    join(directory, `${name}.log`),
    result.stdout + result.stderr,
    {
      flag: "wx",
    },
  );
  assert.equal(result.status, 0, result.stderr);
  const messages: unknown[] = result.stdout
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  const executable = messages.find(
    (message) =>
      isRecord(message) &&
      typeof message.executable === "string" &&
      isRecord(message.target) &&
      message.target.name === name,
  );
  assert.ok(isRecord(executable) && typeof executable.executable === "string");
  binaries.push({
    name,
    path: executable.executable,
    sha256: digest(await readFile(executable.executable)),
  });
}
const profile = binaries.find((binary) => binary.name === "recursive-profile");
assert.ok(profile);
const results = [];
for (const name of [
  "inline-control",
  "tail-12",
  "branching",
  "captured",
  "target-capture",
  "multi-argument",
  "accumulator",
]) {
  const result = JSON.parse(
    execFileSync(profile.path, [name, "1"], { encoding: "utf8" }),
  );
  const expected = JSON.parse(
    execFileSync(resolve(snapshot, "recursive-profile"), [name, "1"], {
      encoding: "utf8",
    }),
  );
  assert.deepEqual(result, expected, name);
  results.push(result);
}
await save("result.json", { status: "complete", binaries, results });
