import { spawnSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { delimiter, join, resolve } from "node:path";

export const runTlc = async (
  java: string,
  tlcJar: string,
  apalacheJar: string,
  directory: string,
  name: string,
  source: string,
  config: string,
  modules: Readonly<Record<string, string>> = {},
) => {
  await mkdir(directory);
  await mkdir(join(directory, "scratch"));
  await writeFile(join(directory, `${name}.tla`), source, { flag: "wx" });
  await writeFile(join(directory, `${name}.cfg`), config, { flag: "wx" });
  for (const [moduleName, moduleSource] of Object.entries(modules))
    await writeFile(join(directory, `${moduleName}.tla`), moduleSource, {
      flag: "wx",
    });
  const args = [
    "-Xmx256m",
    `-Djava.io.tmpdir=${resolve(directory, "scratch")}`,
    "-cp",
    [resolve(tlcJar), resolve(apalacheJar)].join(delimiter),
    "tlc2.TLC",
    "-workers",
    "1",
    "-nowarning",
    "-config",
    `${name}.cfg`,
    `${name}.tla`,
  ];
  const result = spawnSync(resolve(java), args, {
    cwd: directory,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  const log = result.stdout + result.stderr;
  await writeFile(join(directory, "tlc.log"), log, { flag: "wx" });
  await writeFile(
    join(directory, "command.json"),
    JSON.stringify([resolve(java), ...args], null, 2),
    { flag: "wx" },
  );
  const counts = /(\d+) states generated, (\d+) distinct states found/.exec(
    log,
  );
  return {
    status:
      result.status === 0
        ? "complete"
        : /Invariant .*is violated/.test(log)
          ? "invariant-violation"
          : "error",
    exit: result.status,
    states: counts ? Number(counts[2]) : null,
    generated: counts ? Number(counts[1]) : null,
    log,
  };
};
