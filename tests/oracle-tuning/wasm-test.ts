import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { readOracleWasm } from "../../evals/experiments/oracle-tuning/wasm.js";
import { snapshotModule } from "../../evals/experiments/snapshot-module.js";

test("replay loads modules from the layout recorded in the snapshot", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "oracle-layout-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const legacy = join(root, "runtime/evals");
  const current = join(legacy, "experiments");
  const load = () => import(snapshotModule(root, "runtime", "marker.mjs"));

  await mkdir(legacy, { recursive: true });
  await writeFile(join(legacy, "marker.mjs"), 'export default "legacy"');
  assert.equal((await load()).default, "legacy");

  await mkdir(current);
  await assert.rejects(load(), { code: "ERR_MODULE_NOT_FOUND" });
  await writeFile(join(current, "marker.mjs"), 'export default "current"');
  assert.equal((await load()).default, "current");
});

test("a frozen oracle worker runs its own Wasm binding outside the checkout", async () => {
  const root = await mkdtemp(join(tmpdir(), "roracle-worker-"));
  const frozen = join(root, "snapshot");
  try {
    execFileSync(process.execPath, [
      "dist/evals/experiments/oracle-tuning/run.js",
      "freeze",
      frozen,
    ]);
    const output = execFileSync(
      process.execPath,
      [join(frozen, "worker.mjs"), "oracle"],
      {
        cwd: root,
        env: { ...process.env, IDEALIS_ORACLE_BACKEND: "native-json" },
        input: '{"actual":1,"expected":1}\n{"actual":2,"expected":1}\n',
        encoding: "utf8",
      },
    );
    assert.deepEqual(
      output
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line)),
      [{ status: "matched" }, { status: "mismatch" }],
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("frozen oracle snapshots accept current and historical Wasm filenames", async () => {
  const root = await mkdtemp(join(tmpdir(), "roracle-frozen-"));
  const release = "target/wasm32-unknown-unknown/release";
  const historical = join(root, "crates/oracle", release);
  const directory = join(root, "crates/roracle", release);
  try {
    await mkdir(directory, { recursive: true });
    await assert.rejects(readOracleWasm(root), { code: "ENOENT" });
    await mkdir(historical, { recursive: true });
    await writeFile(join(historical, "idealis_oracle.wasm"), "historical");
    assert.equal((await readOracleWasm(root)).toString(), "historical");
    await writeFile(join(historical, "roracle.wasm"), "renamed binary");
    assert.equal((await readOracleWasm(root)).toString(), "renamed binary");
    await writeFile(join(directory, "roracle.wasm"), "current");
    assert.equal((await readOracleWasm(root)).toString(), "current");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
