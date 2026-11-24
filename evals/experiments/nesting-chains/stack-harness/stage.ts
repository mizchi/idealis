import { spawnSync } from "node:child_process";
import { writeFile } from "node:fs/promises";
import { cellFrom, type Cell } from "./cell.js";

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

await writeFile(output, "", { flag: "wx" });

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
    cells.push(cellFrom(family, Number(length), result));
  }
}
await writeFile(output, `${JSON.stringify(cells, null, 1)}\n`);
for (const cell of cells) {
  const first = cell.stderr.split("\n").find((line) => line.length > 0) ?? "";
  console.log(
    cell.family,
    cell.length,
    cell.exit ?? cell.signal ?? "spawn-error",
    cell.stdout || cell.spawnError || first.slice(0, 160),
  );
}
