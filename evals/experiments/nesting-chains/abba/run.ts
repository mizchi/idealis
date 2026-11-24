import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { sampleDirectory, sampleSides, type SampleRecord } from "./samples.js";

type Binaries = Readonly<
  Record<
    "before" | "after",
    Readonly<Record<string, { sha256: string; path: string }>>
  >
>;

const [run, manifestPath, ...names] = process.argv.slice(2);
if (run === undefined || manifestPath === undefined || names.length === 0) {
  console.error("usage: run.js <run directory> <binaries.json> <benchmark>...");
  process.exit(2);
}
const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as Binaries;

let failed = false;
for (const name of names) {
  const base = join(run, name);
  await mkdir(base, { recursive: true });
  const records: SampleRecord[] = [];
  for (const [index, side] of sampleSides.entries()) {
    const binary = manifest[side][name];
    if (binary === undefined)
      throw new Error(`${side}/${name} not in manifest`);
    const digest = createHash("sha256")
      .update(await readFile(binary.path))
      .digest("hex");
    if (digest !== binary.sha256)
      throw new Error(`${side}/${name} sha256 changed`);
    const directory = join(base, sampleDirectory(index, side));
    await mkdir(directory, { recursive: true });
    const environment: NodeJS.ProcessEnv = {
      ...process.env,
      CRITERION_HOME: join(directory, "criterion"),
    };
    delete environment["CARGO_TARGET_DIR"];
    delete environment["CARGO_BUILD_TARGET_DIR"];
    const started = Date.now();
    const result = spawnSync(binary.path, ["--bench", "--noplot"], {
      env: environment,
      encoding: "utf8",
      maxBuffer: 1 << 28,
    });
    const exit = result.status ?? -1;
    await writeFile(join(directory, "stdout.log"), result.stdout ?? "");
    await writeFile(join(directory, "stderr.log"), result.stderr ?? "");
    records.push({ index, side, exit });
    console.log(
      name,
      index,
      side,
      "exit",
      exit,
      ((Date.now() - started) / 1000).toFixed(1),
      "s",
    );
    if (exit !== 0) {
      failed = true;
      break;
    }
  }
  await writeFile(join(base, "runs.json"), JSON.stringify(records, null, 1));
}
process.exit(failed ? 1 : 0);
