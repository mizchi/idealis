import { spawnSync } from "node:child_process";
import { writeFile } from "node:fs/promises";

type Cell = {
  readonly family: string;
  readonly length: number;
  readonly exit: number | null;
  readonly signal: string | null;
  readonly stdout: string;
  readonly stderr: string;
};

const [
  binary,
  output,
  families = "if,flat,case",
  lengths = "3,47,48,61,127,128",
] = process.argv.slice(2);
if (binary === undefined || output === undefined) {
  console.error(
    "usage: stage.js <harness binary> <output.json> [families] [lengths]",
  );
  process.exit(2);
}

const cells: Cell[] = [];
for (const family of families.split(",")) {
  for (const length of lengths.split(",")) {
    const result = spawnSync(
      binary,
      ["chain", family, length, String(1 << 20), "2"],
      {
        encoding: "utf8",
        timeout: 600_000,
      },
    );
    cells.push({
      family,
      length: Number(length),
      exit: result.status,
      signal: result.signal,
      stdout: result.stdout.trim(),
      stderr: result.stderr.trim(),
    });
  }
}
await writeFile(output, `${JSON.stringify(cells, null, 1)}\n`);
for (const cell of cells) {
  const first = cell.stderr.split("\n").find((line) => line.length > 0) ?? "";
  console.log(
    cell.family,
    cell.length,
    cell.exit ?? cell.signal,
    cell.stdout || first.slice(0, 160),
  );
}
