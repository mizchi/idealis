import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { delimiter, join } from "node:path";

export interface TlcState {
  readonly [variable: string]: string;
}

export interface TlcResult {
  readonly exit: number | null;
  readonly signal: string | null;
  readonly timedOut: boolean;
  readonly milliseconds: number;
  readonly verdict: string;
  readonly completed: boolean;
  readonly states: number | null;
  readonly generated: number | null;
  readonly trace: readonly TlcState[];
  readonly errors: readonly string[];
}

export const parseTrace = (log: string): TlcState[] => {
  const states: Record<string, string>[] = [];
  let current: Record<string, string> | null = null;
  for (const line of log.split("\n")) {
    if (/^State \d+:/.test(line)) {
      current = {};
      states.push(current);
      continue;
    }
    const assignment = /^(?:\/\\ )?(\w+) = (.*)$/.exec(line);
    if (current && assignment?.[1] !== undefined && assignment[2] !== undefined)
      current[assignment[1]] = assignment[2];
    else if (line.trim() === "") current = null;
  }
  return states;
};

export const verdictOf = (log: string, timedOut: boolean): string => {
  if (timedOut) return "timeout";
  if (/Model checking completed\. No error has been found/.test(log))
    return "complete";
  if (/Invariant \w+ is violated/.test(log)) return "violation";
  if (/The second argument of \\div is 0|divide by zero/i.test(log))
    return "division-by-zero";
  if (/StackOverflowError/.test(log)) return "stack-overflow";
  return "other-error";
};

if (process.argv[1]?.endsWith("tlc-0120.js")) {
  const [output, casesJson, filter] = process.argv.slice(2);
  const java = process.env.TLC_JAVA;
  const jar = process.env.TLC_JAR;
  assert.ok(
    output && casesJson && java && jar,
    "usage: TLC_JAVA=<java> TLC_JAR=<jar> tlc-0120.js <out dir> <cases.json> [name regexp]",
  );
  mkdirSync(output);
  const cases = JSON.parse(readFileSync(casesJson, "utf8")) as {
    name: string;
    source: string;
    config: string;
  }[];
  const pattern = filter ? new RegExp(filter) : null;
  const results: Record<string, TlcResult> = {};
  for (const entry of cases) {
    if (pattern && !pattern.test(entry.name)) continue;
    const directory = join(output, entry.name.replace(/[^A-Za-z0-9_.-]/g, "_"));
    mkdirSync(directory);
    mkdirSync(join(directory, "scratch"));
    mkdirSync(join(directory, "states"));
    const moduleName = /MODULE\s+(\w+)/.exec(entry.source)?.[1];
    assert.ok(moduleName, entry.name);
    writeFileSync(join(directory, `${moduleName}.tla`), entry.source, {
      flag: "wx",
    });
    writeFileSync(join(directory, `${moduleName}.cfg`), entry.config, {
      flag: "wx",
    });
    const args = [
      "-Xmx256m",
      "-Xss8m",
      "-XX:+UseParallelGC",
      `-Djava.io.tmpdir=${join(directory, "scratch")}`,
      "-cp",
      [jar].join(delimiter),
      "tlc2.TLC",
      "-workers",
      "1",
      "-nowarning",
      "-metadir",
      join(directory, "states"),
      "-config",
      `${moduleName}.cfg`,
      `${moduleName}.tla`,
    ];
    const started = Date.now();
    const run = spawnSync(java, args, {
      cwd: directory,
      encoding: "utf8",
      maxBuffer: 1 << 26,
      timeout: 300_000,
    });
    const log = `${run.stdout ?? ""}${run.stderr ?? ""}`;
    writeFileSync(join(directory, "tlc.log"), log, { flag: "wx" });
    const counts = /(\d+) states generated, (\d+) distinct states found/.exec(
      log,
    );
    const timedOut =
      (run.error as NodeJS.ErrnoException | undefined)?.code === "ETIMEDOUT";
    results[entry.name] = {
      exit: run.status,
      signal: run.signal,
      timedOut,
      milliseconds: Date.now() - started,
      verdict: verdictOf(log, timedOut),
      completed: /Model checking completed\. No error has been found/.test(log),
      states: counts ? Number(counts[2]) : null,
      generated: counts ? Number(counts[1]) : null,
      trace: parseTrace(log),
      errors: [...log.matchAll(/^Error: (.*)$/gm)]
        .map((match) => match[1] ?? "")
        .slice(0, 3),
    };
    console.log(
      entry.name,
      results[entry.name]?.verdict,
      results[entry.name]?.states,
      results[entry.name]?.generated,
    );
  }
  writeFileSync(
    join(output, "results.json"),
    `${JSON.stringify(results, null, 1)}\n`,
    { flag: "wx" },
  );
  console.log(`TLC runs: ${Object.keys(results).length}`);
}
