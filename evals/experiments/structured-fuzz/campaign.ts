import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { createAudit, type Input, type Report } from "./clients.js";
import { fuzzBinary } from "./paths.js";
import { digest } from "./provenance.js";

export interface File {
  readonly target: "raw" | "structured";
  readonly name: string;
  readonly sha256: string;
  readonly base64: string;
}
export interface Record {
  readonly source: "probe" | "corpus";
  readonly input: Input;
  readonly report: Report;
  readonly wasm: readonly unknown[];
}
export const execute = (binary: string, args: readonly string[]) => {
  const result = spawnSync(binary, [...args], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  return {
    binary,
    args,
    status: result.status,
    signal: result.signal,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
    error: result.error?.message ?? null,
  };
};
export const archive = async (directory: string): Promise<readonly File[]> => {
  const files: File[] = [];
  for (const target of ["raw", "structured"] as const) {
    for (const name of (await readdir(join(directory, target))).sort()) {
      const bytes = await readFile(join(directory, target, name));
      files.push({
        target,
        name,
        sha256: digest(bytes),
        base64: bytes.toString("base64"),
      });
    }
  }
  return files;
};
export const writeCorpus = async (
  directory: string,
  files: readonly File[],
) => {
  for (const target of ["raw", "structured"])
    await mkdir(join(directory, target), { recursive: true });
  for (const file of files) {
    assert.match(file.name, /^[a-zA-Z0-9_-]+$/);
    const bytes = Buffer.from(file.base64, "base64");
    assert.equal(digest(bytes), file.sha256);
    await writeFile(join(directory, file.target, file.name), bytes, {
      flag: "wx",
    });
  }
};
export const campaign = (
  target: "raw" | "structured",
  directory: string,
  artifacts: string,
  seed: number,
  runs: number,
) =>
  execute(fuzzBinary(target), [
    directory,
    `-seed=${seed}`,
    `-runs=${runs}`,
    "-max_len=8192",
    "-timeout=0",
    "-rss_limit_mb=1024",
    "-print_final_stats=1",
    "-use_value_profile=1",
    `-artifact_prefix=${artifacts}/`,
  ]);
export const replayAsan = async (
  directory: string,
  files: readonly File[],
  seed: number,
) => {
  await writeCorpus(directory, files);
  const results = [];
  for (const target of ["raw", "structured"] as const) {
    const inputs = files.filter((file) => file.target === target);
    for (let index = 0; index < inputs.length; index += 32) {
      const paths = inputs
        .slice(index, index + 32)
        .map((file) => join(directory, target, file.name));
      const result = execute(fuzzBinary(target), [
        ...paths,
        `-seed=${seed}`,
        "-runs=0",
        "-max_len=8192",
        "-timeout=0",
        "-rss_limit_mb=1024",
      ]);
      assert.equal(result.status, 0, result.stderr);
      results.push(result);
    }
  }
  return results;
};
export const validate = async (
  inputs: readonly { source: "probe" | "corpus"; input: Input }[],
  save: (record: Record) => void,
) => {
  const audit = await createAudit();
  try {
    for (const entry of inputs)
      save({ source: entry.source, ...(await audit.check(entry.input)) });
  } finally {
    await audit.close();
  }
};
export const summarize = (records: readonly Record[]) =>
  ["probe", "corpus"].flatMap((source) =>
    ["raw", "structured"].map((mode) => {
      const selected = records.filter(
        (record) => record.source === source && record.input.mode === mode,
      );
      const phases = [
        "generation",
        "baseline",
        "shrink",
        "confirmation",
        "replay",
      ];
      return {
        source,
        mode,
        inputs: selected.length,
        engineCalls: selected.reduce(
          (sum, record) => sum + record.report.engineCalls,
          0,
        ),
        errors: selected.reduce((sum, record) => sum + record.report.errors, 0),
        bounded: selected.filter((record) => record.report.bounded).length,
        phaseInputs: Object.fromEntries(
          phases.map((phase) => [
            phase,
            selected.filter((record) => (record.report.phases[phase] ?? 0) > 0)
              .length,
          ]),
        ),
        terminalInputs: selected.filter(
          (record) => Object.keys(record.report.results).length > 0,
        ).length,
      };
    }),
  );
